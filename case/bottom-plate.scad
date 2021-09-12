use <plate.scad>

$fa = 1;
$fs = 0.4;

module bottom_plate() {
    difference() {
        plate();

        // Reset/DFU button holes
        translate([86.5, 30])
            circle(r=1);
        translate([86.5, 23.5])
            circle(r=1);
    }
}

bottom_plate();