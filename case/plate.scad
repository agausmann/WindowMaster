// Base plate module used for both bottom & top plates.

width = 110;
height = 60;
radius = 2;
screw_radius = 1.25;
screw_inset = 3.25;

module plate() {
    difference() {
        translate([radius, radius])
            minkowski() {
                square([width - 2 * radius, height - 2 * radius]);
                circle(r=radius);
            }

        translate([screw_inset, screw_inset])
            circle(r=screw_radius);        
        translate([screw_inset, height - screw_inset])
            circle(r=screw_radius);
        translate([width - screw_inset, screw_inset])
            circle(r=screw_radius);
        translate([width - screw_inset, height - screw_inset])
            circle(r=screw_radius);
    }
}

plate();