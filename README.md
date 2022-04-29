
To do:

Remove trait bounds from struct definitions and keep in the impl only.

Move IR implementation to within QDLDL.   Move all IR settings to within QDLDL and then remove lifetime annotation on KKTsolver that are supporting shared references to the master settings object.

Apply builder method to master settings and implement defaults that way instead.

Consider whether SolveResult can be made a dependency only in the same way as Cone in the top level solver.   Maybe not since it depends on descaling equilibration stuff.

Consider whether Settings can be made dependency only as well.   This should be easier since there are minimal direct field accesses in the top level solver.  Probably only max_step_fraction() is needed as a getter in solver to allow a generic settings input by trait there.







Julia compat updates:

Change QDLDL dsign behaviour to be consistent with Rust.   QDLDL should use internally held signs when applying offsets, not an externally supplied vector.

Changed cone_scaling_update call in top level solver to a method scale_cones on the variables.   This way only Variables/Residuals/Data/KKT/SolveInfo/SolveResult need to be mutually interoperable, since the Cone type has no methods taking any of these as arguments.
