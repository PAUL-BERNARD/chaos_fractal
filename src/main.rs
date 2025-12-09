#![allow(unused_variables)]
#![allow(dead_code)]

use std::borrow::Cow;
use std::f32::consts::TAU;
use std::time;

use ffmpeg::codec::traits::Encoder;
use ffmpeg_next as ffmpeg;
use wgpu::util::DeviceExt;
use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    /// Filename of the output file
    #[arg(short, long, default_value_t = {"./FRACTAL.png".to_string()})]
    filename: String,
    /// Number of edges of the polygon
    #[arg(short, long, default_value_t = 5)]
    edges: usize,
    /// Width and height of the output image (in pixels)
    #[arg(short, long, default_value_t = 2048)]
    resolution: usize,
    /// Number of iterations of the chaotic game steps
    #[arg(short, long, default_value_t = 300_000_000)]
    iterations: usize,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Argument parsing
    let args = Args::parse();

    let points = get_vertices(args.resolution, args.edges);

    // Image initialization
    ffmpeg::init().unwrap();

    let mut video_frame = ffmpeg::util::frame::video::Video::new(
        ffmpeg::format::Pixel::RGB24,
        args.resolution.try_into().unwrap(),
        args.resolution.try_into().unwrap()
    );
    video_frame.set_pts(Some(0));

    let stride = video_frame.stride(0);
    let image = video_frame.data_mut(0);

    // Image drawing
    image.fill(255);
    
    draw_image_cpu(image, args.iterations, args.resolution, stride, &points);
    
    // draw_image_gpu(image, stride).await.unwrap();

    // Image saving

    let mut output_ctx = ffmpeg::format::output(&args.filename)
        .unwrap();

    let _output_stream = output_ctx.add_stream(ffmpeg::encoder::find(ffmpeg::codec::Id::PNG).unwrap()).unwrap();

    let encoder_codec : ffmpeg::codec::Video = ffmpeg::encoder::find(ffmpeg::codec::Id::PNG)
        .unwrap()
        .encoder()
        .unwrap()
        .video()
        .unwrap();
    
    let mut encoder_ctx = ffmpeg::codec::Context::new().encoder().video().unwrap();
    encoder_ctx.set_width(args.resolution as u32);
    encoder_ctx.set_height(args.resolution as u32);
    encoder_ctx.set_format(ffmpeg::format::Pixel::RGB24);
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


fn get_vertices(resolution: usize, sides: usize) -> Vec<[usize; 2]> {
    let mut vertices = Vec::with_capacity(sides);
    let radius = 0.9 * ((resolution as f32) / 2.0);
    for i in 0..sides {
        let angle = TAU * (i as f32) / (sides as f32);
        let x = resolution as f32 / 2.0 - radius * angle.cos();
        let y = resolution as f32 / 2.0 + radius * angle.sin();
        vertices.push([x as usize, y as usize]);
    }
    vertices
}

fn fill_polygon(image: &mut [u8], resolution: usize, stride: usize, points: &Vec<[usize; 2]>) {
    for x in 0..resolution {
        for y in 0..resolution {
            if in_polygon(x, y, points) {
                image[x*stride + y] = 255;
            }
        }
    }
}


fn in_polygon(x: usize, y: usize, points: &Vec<[usize; 2]>) -> bool {
    // For each edge [v1, v2], we verify that ([x,y] - v1) × (v2 - v1) > 0
    // (or something like that) 
    for i in 0..points.len() {
        let (v1, v2) = (points[i], points[(i+1)%points.len()]);
        if x*v1[1] + v1[0]*v2[1] + y*v2[0] > x*v2[1] + v1[1]*v2[0] + y*v1[0] {
            return false
        }
    }

    true
}

fn draw_image_cpu(image: &mut [u8], iterations: usize, resolution: usize, stride: usize, points: &Vec<[usize; 2]>) {
    let mut cursor = [resolution/2, resolution/2];
    let mut pixel : usize;

    for _i in 0..iterations {
        change_color(image, cursor, stride);
        // cursor = fern_next(cursor);
        cursor = intermediate(cursor, points[fastrand::usize(0..points.len())]);
    }
}

fn change_color(image: &mut [u8], cursor: [usize; 2], stride: usize) {
    // Cursor pointer
    let p = (cursor[0])*stride + cursor[1]*3;
    image[p] = image[p].saturating_sub(1);
    image[p + 1] = image[p + 1].saturating_sub(1);
    image[p + 2] = image[p + 2].saturating_sub(1);
}

fn intermediate(p1 : [usize; 2], p2 : [usize; 2]) -> [usize; 2] {
    [(p1[0]+p2[0])/2, (p1[1]+p2[1])/2]
}

fn fern_next([x,y] : [usize; 2]) -> [usize; 2] {
    let n = fastrand::u8(0..=u8::MAX);
    match n {
        0..=3     => [ 16*(100+(13*(x-100))%1848)/100+1636, 1024],
        4..=200   => [85*x/100+y*24/1000-28, 283+y*85/100-667*x/10000],
        201..=228 => [22*x/100+1365-y*138/1000, y*20/100+433*x/1000-25],
        229..=255 => [24*x/100+1559-y*156/1000, 2086-y*15/100-467*x/1000],
    }
}

async fn draw_image_gpu(image: &mut [u8], resolution: usize, stride: usize, points: &Vec<[usize; 2]>) -> Option<()> {
    let start = time::Instant::now();
    // image[stride*(IMAGE_SIZE/2) + IMAGE_SIZE/2] = 1;
    // image[stride*(1+IMAGE_SIZE/2) + IMAGE_SIZE/2] = 1;
    // image[stride*(1+IMAGE_SIZE/2) + 1+IMAGE_SIZE/2] = 1;
    // image[stride*(IMAGE_SIZE/2) + 1+IMAGE_SIZE/2] = 1;
    // image.fill(255);
    fill_polygon(image, resolution, stride, points);
    let time =  time::Instant::now() - start;
    println!("fill polygon : {time:?}");

    // Create default WGPU instance
    let instance = wgpu::Instance::default();

    let adapter = instance
        .request_adapter(
            &wgpu::RequestAdapterOptions::default()
        )
        .await.ok()?;

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Device descriptor"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                .. Default::default()
            }
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
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    let mut command_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Command encoder"),
    });


    let time =  time::Instant::now() - start;
    println!("Created bordel : {time:?}");

    for i in 0..300 {
        {
            let mut compute_pass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(&format!("Compute pass {}", &i)),
                .. Default::default()
            });

            compute_pass.set_pipeline(&pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            compute_pass.insert_debug_marker(&format!("Compute pass {}", &i));
            compute_pass.dispatch_workgroups((resolution/8) as u32, (resolution/(4*8)) as u32, 1);
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
    let time =  time::Instant::now() - start;
    println!("iterated pipeline : {time:?}");

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

    let _ = device.poll(wgpu::wgt::PollType::Wait { submission_index: None, timeout: None });

    let data = buffer_slice.get_mapped_range().to_vec();

    putain_buffer.unmap();

    image.copy_from_slice(&data[..]);

    Some(())
}

