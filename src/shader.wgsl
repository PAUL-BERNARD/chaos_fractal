@group(0) @binding(0) var<storage, read> image: array<u32>;
@group(0) @binding(1) var<storage, read_write> result: array<u32>;


fn get_image_at(x: u32, y: u32) -> u32 {
    if (x>=512u || y>=512u) {
        return 0u;
    }

    return (image[x*128u + (y/4u)] >> (8u*(y%4u))) % 256u;
}

fn get_value_at(x: u32, y: u32) -> u32 {
    return (
        get_image_at(x, y) +
        get_image_at(x, y+1u) +
        get_image_at(x+1u, y+1u) +
        get_image_at(x+1u, y)
    );
}


fn draw_points_u8(x: u32, y: u32) -> u32 {
    var vertices_x = array(26u, 185u, 442u, 442u, 185u);
    var vertices_y = array(256u, 475u, 391u, 121u, 37u);

    var val: u32 = 0u;
    for (var i:u32 = 0u; i < 5u; i++) {
        if (2u*x >= vertices_x[i] && 2u*y >= vertices_y[i]) {
            val += get_value_at(
                x + x - vertices_x[i],
                y + y - vertices_y[i]
            );
        }
    }

    // if (val > 0u) { return 255u;} else {return 0u;}
    return min(val/5u, 255u);
}

fn draw_points_u32(x: u32, y: u32) -> u32 {
    return
         draw_points_u8(x, 4u*y) +
        (draw_points_u8(x, 4u*y+1u) << 8u ) +
        (draw_points_u8(x, 4u*y+2u) << 16u) +
        (draw_points_u8(x, 4u*y+3u) << 24u) ;
}

@compute
@workgroup_size(8,8)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    result[global_id.x*128u + global_id.y] = draw_points_u32(global_id.x, global_id.y);
}
