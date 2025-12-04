@group(0) @binding(0) var<storage, read> image: array<u32>;
@group(0) @binding(1) var<storage, read_write> result: array<u32>;


fn get_image_at(x: u32, y: u32) -> u32 {
    if (x>=2048u || y>=2048u) {
        return 0u;
    }

    return (image[x*512u + (y/4u)] >> (8u*(y%4u))) % 256u;
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
    var vertices_x = array(102u, 449u, 1229u, 1854u, 1854u, 1229u, 449u);
    var vertices_y = array(1024u, 1745u, 1922u, 1424u, 624u, 126u, 303u);

    var val: u32 = 0u;
    for (var i:u32 = 0u; i < 5u; i++) {
        if (2u*x >= vertices_x[i] && 2u*y >= vertices_y[i]) {
            val += get_value_at(
                x + x - vertices_x[i],
                y + y - vertices_y[i]
            );
        }
    }

    return min(val/5u, 255u);
}

fn draw_points_u32(x: u32, y: u32) -> u32 {
    return
         draw_points_u8(x, 4u*y) +
        (draw_points_u8(x, 4u*y+1u) << 8u ) +
        (draw_points_u8(x, 4u*y+2u) << 16u) +
        (draw_points_u8(x, 4u*y+3u) << 24u) ;
}

fn draw_xd(x: u32, y: u32) -> u32 {
    return
        0u +
        (100u << 8u ) +
        (200u << 16u) +
        (255u << 24u) ;
}

@compute
@workgroup_size(8,8)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    // result[global_id.x*512u + global_id.y] = draw_points_u32(global_id.x, global_id.y);
    result[global_id.x*512u + global_id.y] = draw_xd(global_id.x, global_id.y);
}
