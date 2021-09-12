use <plate.scad>

$fa = 1;
$fs = 0.4;

ch1_encoder_x = 17.7;
ch1_encoder_y = 41;
encoder_radius = 3.6;
ch1_led_x = 29;
ch1_led_y = 35.725;
led_radius = 1.6;
channel_col_spacing = 30;
channel_row_spacing = 22;
channel_rows = 2;
channel_cols = 3;

module top_plate() {
    difference() {
        plate();

        // ST-Link header
        translate([65.5, 50.4])
            square([15.7, 3.0]);

        // Channel cutouts
        for (row = [0:channel_rows - 1]) {
            for (col = [0:channel_cols - 1]) {
                translate([
                    channel_col_spacing * col,
                    -channel_row_spacing * row,
                ]) {
                    translate([ch1_encoder_x, ch1_encoder_y])
                        circle(r=encoder_radius);
                    translate([ch1_led_x, ch1_led_y])
                        circle(r=led_radius);
                }
            }
        }
    }
}

top_plate();