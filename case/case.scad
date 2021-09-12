// A full model (including the PCB model) to demonstrate the assembled case
// and visually verify the plate drawings.
//
// To order parts for this case, use the `top-plate` and `bottom-plate` files
// instead of this model.

use <top-plate.scad>
use <bottom-plate.scad>

plate_thickness = 2;
standoff_height = 15;

module case() {
    color([1, 1, 1, 0.2]) {
        linear_extrude(plate_thickness)
            top_plate();
        translate([0, 0, -(standoff_height + plate_thickness)])
            linear_extrude(plate_thickness)
                bottom_plate();
    }
    translate([55, 30, -8])
        import("windowmaster.stl");
}

case();