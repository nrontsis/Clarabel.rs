use crate::algebra::*;
use std::time::Duration;

use super::*;
use crate::solver::core::{
    cones::{CompositeCone, SupportedCones},
    traits::InfoPrint,
};

macro_rules! expformat {
    ($fmt:expr,$val:expr) => {
        if $val.is_finite() {
            _exp_str_reformat(format!($fmt, $val))
        } else {
            format!($fmt, $val)
        }
    };
}

impl<T> InfoPrint<T> for DefaultInfo<T>
where
    T: FloatT,
{
    type D = DefaultProblemData<T>;
    type C = CompositeCone<T>;
    type SE = DefaultSettings<T>;

    fn print_configuration(
        &self,
        settings: &DefaultSettings<T>,
        data: &DefaultProblemData<T>,
        cones: &CompositeCone<T>,
    ) {
        if !settings.verbose {
            return;
        }

        println!("\nproblem:");
        println!("  variables     = {}", data.n);
        println!("  constraints   = {}", data.m);
        println!("  nnz(P)        = {}", data.P.nnz());
        println!("  nnz(A)        = {}", data.A.nnz());
        println!("  cones (total) = {}", cones.len());

        //All dims here are dummies since we just care about the cone type
        _print_conedims_by_type(cones, SupportedCones::ZeroConeT(0));
        _print_conedims_by_type(cones, SupportedCones::NonnegativeConeT(0));
        _print_conedims_by_type(cones, SupportedCones::SecondOrderConeT(0));
        //_print_conedims_by_type(&cones, SupportedCones::PSDTriangleConeT(0));

        println!();
        _print_settings(settings);
        println!();
    }

    fn print_status_header(&self, settings: &DefaultSettings<T>) {
        if !settings.verbose {
            return;
        }

        //print a subheader for the iterations info
        print!("iter    ");
        print!("pcost        ");
        print!("dcost       ");
        print!("pres      ");
        print!("dres      ");
        print!("k/t       ");
        print!(" μ       ");
        print!("step      ");
        println!();
        println!(
            "-----------------------------------------------------------------------------------"
        );
    }

    fn print_status(&self, settings: &DefaultSettings<T>) {
        if !settings.verbose {
            return;
        }

        print!("{:>3}  ", self.iterations);
        print!("{}  ", expformat!("{:+8.4e}", self.cost_primal));
        print!("{}  ", expformat!("{:+8.4e}", self.cost_dual));
        print!("{}  ", expformat!("{:6.2e}", self.res_primal));
        print!("{}  ", expformat!("{:6.2e}", self.res_dual));
        print!("{}  ", expformat!("{:6.2e}", self.ktratio));
        print!("{}  ", expformat!("{:6.2e}", self.μ));

        if self.iterations > 0 {
            print!("{}  ", expformat!("{:>.2e}", self.step_length));
        } else {
            print!(" ------   "); //info.step_length
        }

        println!();
    }

    fn print_footer(&self, settings: &DefaultSettings<T>) {
        if !settings.verbose {
            return;
        }

        println!(
            "-----------------------------------------------------------------------------------"
        );

        println!("Terminated with status = {}", self.status);

        println!("solve time = {:?}", self.solve_time);
    }
}

fn _bool_on_off(v: bool) -> &'static str {
    match v {
        true => "on",
        false => "false",
    }
}

fn _print_settings<T: FloatT>(settings: &DefaultSettings<T>) {
    let set = settings;

    println!("settings:");

    if set.direct_kkt_solver {
        println!(
            "  linear algebra: direct / {}, precision: {} bit",
            set.direct_solve_method,
            _get_precision_string::<T>()
        );
    }

    let time_lim_str = {
        if set.time_limit == Duration::MAX {
            "Inf".to_string()
        } else {
            format!("{:?}", set.time_limit)
        }
    };
    println!(
        "  max iter = {}, time limit = {},  max step = {:.3}",
        set.max_iter, time_lim_str, set.max_step_fraction
    );

    println!(
        "  tol_feas = {:.1e}, tol_abs = {:.1e}, tol_rel = {:.1e},",
        set.tol_feas, set.tol_gap_abs, set.tol_gap_rel
    );

    println!(
        "  static reg : {}, ϵ = {:.1e}",
        _bool_on_off(set.static_regularization_enable),
        set.static_regularization_eps
    );

    println!(
        "  dynamic reg: {}, ϵ = {:.1e}, δ = {:.1e}",
        _bool_on_off(set.dynamic_regularization_enable),
        set.dynamic_regularization_eps,
        set.dynamic_regularization_delta
    );

    println!(
        "  iter refine: {}, reltol = {:.1e}, abstol = {:.1e},",
        _bool_on_off(set.iterative_refinement_enable),
        set.iterative_refinement_reltol,
        set.iterative_refinement_abstol
    );

    println!(
        "               max iter = {}, stop ratio = {:.1}",
        set.iterative_refinement_max_iter, set.iterative_refinement_stop_ratio
    );

    println!(
        "  equilibrate: {}, min_scale = {:.1e}, max_scale = {:.1e}",
        _bool_on_off(set.equilibrate_enable),
        set.equilibrate_min_scaling,
        set.equilibrate_max_scaling
    );

    println!("               max iter = {}", set.equilibrate_max_iter,);
}

fn _get_precision_string<T: FloatT>() -> String {
    (::std::mem::size_of::<T>() * 8).to_string()
}

fn _print_conedims_by_type<T: FloatT>(cones: &CompositeCone<T>, conetype: SupportedCones<T>) {
    let maxlistlen = 5;

    //skip if there are none of this type
    if !cones.type_counts.contains_key(&conetype.variant_name()) {
        return;
    }

    // how many of this type of cone?
    let name = conetype.variant_name();
    let count = cones.type_counts[name];

    //let name  = rpad(string(type)[1:end-5],11)  #drops "ConeT part"
    let name = &name[0..name.len() - 5];
    let name = format!("{:>11}", name);

    let mut nvars = Vec::with_capacity(count);
    for (i, cone) in cones.iter().enumerate() {
        if cones.types[i] == conetype {
            nvars.push(cone.numel());
        }
    }
    print!("    : {} = {}, ", name, count);

    if count == 1 {
        print!(" numel = {}", nvars[0]);
    } else if count <= maxlistlen {
        //print them all
        print!(" numel = (");
        for nvar in nvars.iter().take(nvars.len() - 1) {
            print!("{},", nvar);
        }
        print!("{})", nvars[nvars.len() - 1]);
    } else {
        // print first (maxlistlen-1) and the final one
        print!(" numel = (");
        for nvar in nvars.iter().take(maxlistlen - 1) {
            print!("{},", nvar);
        }
        print!("...,{})", nvars[nvars.len() - 1]);
    }

    println!();
}

// convert a string in LowerExp display format into one that
// 1) always has a sign after the exponent, and
// 2) has at least two digits in the exponent.
// This matches the Julia output formatting.

fn _exp_str_reformat(mut thestr: String) -> String {
    // Safe to `unwrap` as `num` is guaranteed to contain `'e'`
    let eidx = thestr.find('e').unwrap();
    let has_sign = thestr.chars().nth(eidx + 1).unwrap() == '-';

    let has_short_exp = {
        if !has_sign {
            thestr.len() == eidx + 2
        } else {
            thestr.len() == eidx + 3
        }
    };

    let chars;
    if !has_sign {
        if has_short_exp {
            chars = "+0";
        } else {
            chars = "+";
        }
    } else if has_short_exp {
        chars = "0";
    } else {
        chars = "";
    }

    let shift = if has_sign { 2 } else { 1 };
    thestr.insert_str(eidx + shift, chars);
    thestr
}
