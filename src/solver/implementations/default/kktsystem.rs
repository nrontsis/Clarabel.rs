use super::*;
use crate::solver::core::{
    cones::{CompositeCone, Cone},
    kktsolvers::{direct::*, *},
    traits::{KKTSystem, Settings},
};

use crate::algebra::*;

// We require Send here to allow pyo3 builds to share
// solver objects between threads.

type BoxedKKTSolver<T> = Box<dyn KKTSolver<T> + Send>;

/// Standard-form solver type implementing the [KKTSystem](crate::solver::core::traits::KKTSystem) trait

pub struct DefaultKKTSystem<T> {
    kktsolver: BoxedKKTSolver<T>,

    // solution vector for constant part of KKT solves
    x1: Vec<T>,
    z1: Vec<T>,

    // solution vector for general KKT solves
    x2: Vec<T>,
    z2: Vec<T>,

    // work vectors for assembling/dissambling vectors
    workx: Vec<T>,
    workz: Vec<T>,
    work_conic: Vec<T>,
}

impl<T> DefaultKKTSystem<T>
where
    T: FloatT,
{
    pub fn new(
        data: &DefaultProblemData<T>,
        cones: &CompositeCone<T>,
        settings: &DefaultSettings<T>,
    ) -> Self {
        let (m, n) = (data.m, data.n);

        //here we allow scope for different KKT solvers, e.g.
        //direct vs indirect, different QR based direct methods
        //etc.   For now, we only have direct / LDL based
        let kktsolver = if settings.direct_kkt_solver {
            Box::new(DirectLDLKKTSolver::<T>::new(
                &data.P,
                &data.A,
                cones,
                m,
                n,
                settings.core(),
            ))
        } else {
            panic!("Indirect and other solve strategies not yet supported.");
        };

        //the LHS constant part of the reduced solve
        let x1 = vec![T::zero(); n];
        let z1 = vec![T::zero(); m];

        //the LHS for other solves
        let x2 = vec![T::zero(); n];
        let z2 = vec![T::zero(); m];

        //workspace compatible with (x,z)
        let workx = vec![T::zero(); n];
        let workz = vec![T::zero(); m];

        //additional conic workspace vector compatible with s and z
        let work_conic = vec![T::zero(); m];

        Self {
            kktsolver,
            x1,
            z1,
            x2,
            z2,
            workx,
            workz,
            work_conic,
        }
    }
}

impl<T> KKTSystem<T> for DefaultKKTSystem<T>
where
    T: FloatT,
{
    type D = DefaultProblemData<T>;
    type V = DefaultVariables<T>;
    type C = CompositeCone<T>;
    type SE = DefaultSettings<T>;

    fn update(
        &mut self,
        data: &DefaultProblemData<T>,
        cones: &CompositeCone<T>,
        settings: &DefaultSettings<T>,
    ) {
        // update the linear solver with new cones
        self.kktsolver.update(cones, settings.core());

        // calculate KKT solution for constant terms
        self.solve_constant_rhs(data, settings.core());
    }

    fn solve(
        &mut self,
        lhs: &mut DefaultVariables<T>,
        rhs: &DefaultVariables<T>,
        data: &DefaultProblemData<T>,
        variables: &DefaultVariables<T>,
        cones: &CompositeCone<T>,
        steptype: &'static str,
        settings: &DefaultSettings<T>,
    ) {
        let (x1, z1) = (&mut self.x1, &mut self.z1);
        let (x2, z2) = (&self.x2, &self.z2); //from constant solve, so not mut
        let (workx, workz) = (&mut self.workx, &mut self.workz);

        // solve for (x1,z1)
        // -----------
        workx.copy_from(&rhs.x);

        // compute Wᵀ(λ \ ds), with shortcut in affine case
        let Wtlinvds = &mut self.work_conic;

        match steptype {
            "affine" => {
                Wtlinvds.copy_from(&variables.s);
            }
            "combined" => {
                // :combined expected, but any general RHS should do this
                // we can use the overall LHS output as
                // additional workspace for the moment
                let tmp = &mut lhs.z;
                tmp.copy_from(&rhs.z); //Don't want to modify our RHS
                cones.λ_inv_circ_op(tmp, &rhs.s); //tmp = λ \ ds
                cones.gemv_W(MatrixShape::T, tmp, Wtlinvds, T::one(), T::zero());
                // Wᵀ(λ \ ds) = Wᵀ(tmp)
            }
            _ => {
                panic!("Bad step direction specified");
            }
        }

        workz.waxpby(T::one(), Wtlinvds, -T::one(), &rhs.z);

        // this solves the variable part of reduced KKT system
        self.kktsolver.setrhs(workx, workz);
        self.kktsolver.solve(Some(x1), Some(z1), settings.core());

        // solve for Δτ.
        // -----------
        // Numerator first
        let ξ = workx;
        ξ.axpby(T::recip(variables.τ), &variables.x, T::zero());

        let two = T::from(2.).unwrap();
        let tau_num = rhs.τ - rhs.κ / variables.τ
            + data.q.dot(x1)
            + data.b.dot(z1)
            + two * data.P.quad_form(ξ, x1);

        // offset ξ for the quadratic form in the denominator
        let ξ_minus_x2 = ξ; //alias to ξ, same as workx
        ξ_minus_x2.axpby(-T::one(), x2, T::one());

        let mut tau_den = variables.κ / variables.τ - data.q.dot(x2) - data.b.dot(z2);
        tau_den += data.P.quad_form(ξ_minus_x2, ξ_minus_x2) - data.P.quad_form(x2, x2);

        // solve for (Δx,Δz)
        // -----------
        lhs.τ = tau_num / tau_den;
        lhs.x.waxpby(T::one(), x1, lhs.τ, x2);
        lhs.z.waxpby(T::one(), z1, lhs.τ, z2);

        // solve for Δs = -Wᵀ(λ \ dₛ + WΔz) = -Wᵀ(λ \ dₛ) - WᵀWΔz
        // where the first part is already in work_conic
        // -------------
        cones.gemv_W(MatrixShape::N, &lhs.z, workz, T::one(), T::zero()); // work = WΔz
        cones.gemv_W(MatrixShape::T, workz, &mut lhs.s, -T::one(), T::zero()); //Δs = -WᵀWΔz
        lhs.s.axpby(-T::one(), Wtlinvds, T::one()); // s .= -Wtlinvds;

        // solve for Δκ
        // --------------
        lhs.κ = -(rhs.κ + variables.κ * lhs.τ) / variables.τ;
    }

    fn solve_initial_point(
        &mut self,
        variables: &mut DefaultVariables<T>,
        data: &DefaultProblemData<T>,
        settings: &DefaultSettings<T>,
    ) {
        // solve with [0;b] as a RHS to get (x,s) initializers
        // zero out any sparse cone variables at end
        self.workx.fill(T::zero());
        self.workz.copy_from(&data.b);
        self.kktsolver.setrhs(&self.workx, &self.workz);
        self.kktsolver.solve(
            Some(&mut variables.x),
            Some(&mut variables.s),
            settings.core(),
        );

        // solve with [-c;0] as a RHS to get z initializer
        // zero out any sparse cone variables at end
        self.workx.axpby(-T::one(), &data.q, T::zero());
        self.workz.fill(T::zero());

        self.kktsolver.setrhs(&self.workx, &self.workz);
        self.kktsolver
            .solve(None, Some(&mut variables.z), settings.core());
    }
}

impl<T> DefaultKKTSystem<T>
where
    T: FloatT,
{
    fn solve_constant_rhs(&mut self, data: &DefaultProblemData<T>, settings: &DefaultSettings<T>) {
        self.workx.axpby(-T::one(), &data.q, T::zero()); //workx .= -q
        self.kktsolver.setrhs(&self.workx, &data.b);
        self.kktsolver
            .solve(Some(&mut self.x2), Some(&mut self.z2), settings.core());
    }
}
