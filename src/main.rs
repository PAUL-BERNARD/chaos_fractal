#![allow(unused_variables)]
#![allow(dead_code)]

use std::borrow::Cow;

use ffmpeg::codec::traits::Encoder;
use ffmpeg_next as ffmpeg;
use fastrand;
use wgpu::util::DeviceExt;
use tokio;


const IMAGE_SIZE : usize = 2048;
//const POINTS : [[usize; 2]; 7] = [[102, 1024], [449, 1745], [1229, 1922], [1854, 1424], [1854, 624], [1229, 126], [449, 303]];
// const POINTS : [[usize; 2]; 4] = [[0, 0], [0, 512], [512, 512], [512, 0]];
const POINTS : [[usize; 2]; 6] = [[450, 1316], [944, 1564], [822, 1332], [1040, 1738], [1492, 1372], [1294, 522]];
const ITER : usize = 230_000_000;
const OUTPUT_FILE : &str = "./FRACTAL.png";


#[tokio::main(flavor = "current_thread")]
async fn main() {

    ffmpeg::init().unwrap();

    let mut video_frame = ffmpeg::util::frame::video::Video::new(
        ffmpeg::format::Pixel::GRAY8,
        IMAGE_SIZE.try_into().unwrap(),
        IMAGE_SIZE.try_into().unwrap()
    );
    video_frame.set_pts(Some(0));

    let stride = dbg!(video_frame.stride(0));
    let image = video_frame.data_mut(0);
    
    draw_image_cpu(image, stride);
    
    //draw_image_gpu(image, stride).await.unwrap();

    let mut output_ctx = ffmpeg::format::output(&OUTPUT_FILE)
        .unwrap();

    let _ = output_ctx.add_stream(ffmpeg::encoder::find(ffmpeg::codec::Id::PNG).unwrap()).unwrap();

    let encoder_codec : ffmpeg::codec::Video = ffmpeg::encoder::find(ffmpeg::codec::Id::PNG).unwrap().encoder().unwrap().video().unwrap();
    
    let mut encoder_ctx = ffmpeg::codec::Context::new().encoder().video().unwrap();
    encoder_ctx.set_width(IMAGE_SIZE as u32);
    encoder_ctx.set_height(IMAGE_SIZE as u32);
    encoder_ctx.set_format(ffmpeg::format::Pixel::GRAY8);
    encoder_ctx.set_time_base((1,1));

    let mut encoder = encoder_ctx.open_as(encoder_codec).unwrap();

    encoder.send_frame(&video_frame).unwrap();
    encoder.send_eof().unwrap();

    output_ctx.write_header().unwrap();

    let mut encoded_image = ffmpeg::Packet::empty();
    while encoder.receive_packet(&mut encoded_image).is_ok() {
        encoded_image.write(&mut output_ctx).unwrap();
    }
    output_ctx.write_trailer().unwrap();

}

fn fill_polygon(image: &mut [u8], stride: usize) {
    for x in 0..IMAGE_SIZE {
        for y in 0..IMAGE_SIZE {
            if in_polygon(x, y) {
                image[x*stride + y] = 255;
            }
        }
    }
}


fn in_polygon(x: usize, y: usize) -> bool {
    for i in 0..POINTS.len() {
        if 
            x*POINTS[i][1] + POINTS[i][0]*POINTS[(i+1)%POINTS.len()][1] + y*POINTS[(i+1)%POINTS.len()][0]
            >
            x*POINTS[(i+1)%POINTS.len()][1] + POINTS[i][1]*POINTS[(i+1)%POINTS.len()][0] + y*POINTS[i][0]
            {
                return false
            }
    }

    true
}

fn draw_image_cpu(image: &mut [u8], stride: usize) {

    //let mut cursor = [IMAGE_SIZE/2, IMAGE_SIZE/2];
    let mut cursor = [IMAGE_SIZE/2, IMAGE_SIZE];
    //let mut triangle : bool = false;

    for _i in 0..ITER {
        image[(cursor[0])*stride + cursor[1]] = 
            image[cursor[0]*stride + cursor[1]]
                .checked_add(1)
                .unwrap_or(u8::MAX);

        // cursor = intermediate(cursor, POINTS[fastrand::usize(0..POINTS.len())]);
        cursor = fern_next(cursor)
    }
}

fn intermediate(p1 : [usize; 2], p2 : [usize; 2]) -> [usize; 2] {
    return [(p1[0]+p2[0])/2, (p1[1]+p2[1])/2];
}

fn fern_next([x,y] : [usize; 2]) -> [usize; 2] {
    let n = fastrand::u8(0..=u8::MAX);
    match n {
        0..=2 =>     [16*x/100+1720, 1024],
        3..=200 =>   [85*x/100+y*3/100-45, y*85/100+290-6*x/100],
        201..=228 => [22*x/100+1411-y*13/100, y*20/100+43*x/100-68],
        229..=255 => [24*x/100+1626-y*15/100, 2133-y*15/100-46*x/100],
    }
}

async fn draw_image_gpu(image: &mut [u8], stride: usize) -> Option<()> {
    // image[stride*(IMAGE_SIZE/2) + IMAGE_SIZE/2] = 1;
    // image[stride*(1+IMAGE_SIZE/2) + IMAGE_SIZE/2] = 1;
    // image[stride*(1+IMAGE_SIZE/2) + 1+IMAGE_SIZE/2] = 1;
    // image[stride*(IMAGE_SIZE/2) + 1+IMAGE_SIZE/2] = 1;
    // image.fill(255);
    fill_polygon(image, stride);

    // Create default WGPU instance
    let instance = wgpu::Instance::default();

    let adapter = instance
        .request_adapter(
            &wgpu::RequestAdapterOptions::default()
        )
        .await?;

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Device descriptor"),
                features: wgpu::Features::empty(),
                limits: wgpu::Limits::downlevel_defaults()
            },
            None
        ).await.unwrap();

    let image_length = image.len() as wgpu::BufferAddress;

    let image_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Image buffer"),
        contents: image,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST
    });

    let result_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Storage buffer"),
        size: image_length,
        usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false,
    });

    let putain_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("PUTAIN buffer"),
        size: image_length,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Computer shader module"),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl")))
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Bind group layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer { 
                    ty: wgpu::BufferBindingType::Storage {read_only: true}, 
                    has_dynamic_offset: false, 
                    min_binding_size: None 
                },
                count: None,
                binding: 0,
            },
            wgpu::BindGroupLayoutEntry {
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer { 
                    ty: wgpu::BufferBindingType::Storage {read_only: false}, 
                    has_dynamic_offset: false, 
                    min_binding_size: None 
                },
                count: None,
                binding: 1,
            },
        ]

    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Bind group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: image_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: result_buffer.as_entire_binding(),
            },
        ],
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Compute pipeline"),
        layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            bind_group_layouts: &[&bind_group_layout],
            label: Some("Pipeline layout"),
            push_constant_ranges: &[],
        })),
        module: &shader_module,
        entry_point: "main",
    });

    let mut command_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Command encoder"),
    });

    for i in 0..30 {
        {
            let mut compute_pass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(&format!("Compute pass {}", &i)),
            });

            compute_pass.set_pipeline(&pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            compute_pass.insert_debug_marker(&format!("Compute pass {}", &i));
            compute_pass.dispatch_workgroups((IMAGE_SIZE/8) as u32, (IMAGE_SIZE/(4*8)) as u32, 1);
        }
        {
            command_encoder.copy_buffer_to_buffer(
                &result_buffer,
                0,
                &image_buffer,
                0,
                image_length
            );
        }
    }

    command_encoder.copy_buffer_to_buffer(
        &result_buffer,
        0,
        &putain_buffer,
        0,
        image_length
    );

    queue.submit(Some(command_encoder.finish()));

    let buffer_slice = putain_buffer.slice(..);

    buffer_slice.map_async(wgpu::MapMode::Read, move |v| {
        if v.is_err() { println!("{:?}", v) }
    });

    device.poll(wgpu::Maintain::Wait);

    let data = buffer_slice.get_mapped_range().to_vec();

    putain_buffer.unmap();

    image.copy_from_slice(&data[..]);

    Some(())
}

