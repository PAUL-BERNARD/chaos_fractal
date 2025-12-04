use std::borrow::Cow;

use ffmpeg::codec::traits::Encoder;
use ffmpeg_next as ffmpeg;
use fastrand;
use wgpu::util::DeviceExt;
use tokio;


fn intermediate(p1 : [usize; 2], p2 : [usize; 2]) -> [usize; 2] {
    return [(p1[0]+p2[0])/2, (p1[1]+p2[1])/2];
}

async fn run() {
    let numbers = vec![1, 2, 3, 4];

    let steps = execute_gpu(&numbers).await.unwrap();

    println!("Steps: {:#?}", steps);
}

async fn execute_gpu(numbers: &[u32]) -> Option<Vec<u32>> {
    // Instantiates instance of WebGPU
    let instance = wgpu::Instance::default();

    // `request_adapter` instantiates the general connection to the GPU
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await?;

    // `request_device` instantiates the feature specific connection to the GPU, defining some parameters,
    //  `features` being the available features.
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: wgpu::Features::empty(),
                limits: wgpu::Limits::downlevel_defaults(),
            },
            None,
        )
        .await
        .unwrap();

    let info = adapter.get_info();
    // skip this on LavaPipe temporarily
    if info.vendor == 0x10005 {
        println!("Skipped on LavaPipe");
        return None;
    }

    execute_gpu_inner(&device, &queue, numbers).await
}

async fn execute_gpu_inner(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    numbers: &[u32],
) -> Option<Vec<u32>> {
    // Loads the shader from WGSL
    let cs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
    });

    // Gets the size in bytes of the buffer.
    let slice_size = numbers.len() * std::mem::size_of::<u32>();
    let size = slice_size as wgpu::BufferAddress;

    // Instantiates buffer without data.
    // `usage` of buffer specifies how it can be used:
    //   `BufferUsages::MAP_READ` allows it to be read (outside the shader).
    //   `BufferUsages::COPY_DST` allows it to be the destination of the copy.
    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let numbers_pointer : *const u8 = numbers.as_ptr() as *const u8;
    let content_vec = unsafe { std::slice::from_raw_parts(numbers_pointer, numbers.len() * std::mem::size_of::<u32>()) };
    // Instantiates buffer with data (`jjj`).
    // Usage allowing the buffer to be:
    //   A storage buffer (can be bound within a bind group and thus available to a shader).
    //   The destination of a copy.
    //   The source of a copy.
    println!("{:?}", content_vec);
    let storage_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Storage Buffer"),
        contents: content_vec,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
    });

    // A bind group defines how buffers are accessed by shaders.
    // It is to WebGPU what a descriptor set is to Vulkan.
    // `binding` here refers to the `binding` of a buffer in the shader (`layout(set = 0, binding = 0) buffer`).

    // A pipeline specifies the operation of a shader

    // Instantiates the pipeline.
    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &cs_module,
        entry_point: "main",
    });

    // Instantiates the bind group, once again specifying the binding of buffers.
    let bind_group_layout = compute_pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: storage_buffer.as_entire_binding(),
        }],
    });

    // A command encoder executes one or many pipelines.
    // It is to WebGPU what a command buffer is to Vulkan.
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });
        cpass.set_pipeline(&compute_pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.insert_debug_marker("compute collatz iterations");
        cpass.dispatch_workgroups(numbers.len() as u32, 1, 1); // Number of cells to run, the (x,y,z) size of item being processed
    }
    println!("{}, {}", &storage_buffer.size(), &staging_buffer.size());
    // Sets adds copy operation to command encoder.
    // Will copy data from storage buffer on GPU to staging buffer on CPU.
    encoder.copy_buffer_to_buffer(&storage_buffer, 0, &staging_buffer, 0, size);

    // Submits command encoder for processing
    queue.submit(Some(encoder.finish()));

    // Note that we're not calling `.await` here.
    let buffer_slice = staging_buffer.slice(..);
    // Sets the buffer up for mapping, sending over the result of the mapping back to us when it is finished.
    buffer_slice.map_async(wgpu::MapMode::Read, move |v| {
        if v.is_err() { panic!("failed to run compute on gpu!") }
    });

    // Poll the device in a blocking manner so that our future resolves.
    // In an actual application, `device.poll(...)` should
    // be called in an event loop or on another thread.
    device.poll(wgpu::Maintain::Wait);
    // Gets contents of buffer
    let data = buffer_slice.get_mapped_range();
    // Since contents are got in bytes, this converts these bytes back to u32

    let numbers_pointer : *const u32 = (&data).as_ptr() as *const u32;
    let result = unsafe { std::slice::from_raw_parts(numbers_pointer, numbers.len()) }.to_vec();
    
    // let result = data.into_iter().map(|n| *n as u32).collect();

    // With the current interface, we have to make sure all mapped views are
    // dropped before we unmap the buffer.
    drop(data);
    staging_buffer.unmap(); // Unmaps buffer from memory
                            // If you are familiar with C++ these 2 lines can be thought of similarly to:
                            //   delete myPointer;
                            //   myPointer = NULL;
                            // It effectively frees the memory

    // Returns data from buffer
    Some(result)
    
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    const IMAGE_SIZE : usize = 500;
    const POINTS : [[usize; 2]; 6] = [[25, 250], [137, 445], [362, 445], [475, 250], [363, 55], [137, 55]];
    const ITER : usize = 1_000_000;
    const OUTPUT_FILE : &str = "./FRACTAL.png";

    ffmpeg::init().unwrap();

    let mut video_frame = ffmpeg::util::frame::video::Video::new(
        ffmpeg::format::Pixel::GRAY8,
        IMAGE_SIZE.try_into().unwrap(),
        IMAGE_SIZE.try_into().unwrap()
    );
    video_frame.set_pts(Some(0));

    let stride = video_frame.stride(0);
    let image = video_frame.data_mut(0);

    let mut cursor = [IMAGE_SIZE/2, IMAGE_SIZE/2];

    for _i in 0..ITER {
        image[cursor[0]*stride + cursor[1]] = 
            image[cursor[0]*stride + cursor[1]]
                .checked_add(1)
                .unwrap_or(u8::MAX);

        cursor = intermediate(cursor, POINTS[fastrand::usize(0..POINTS.len())]);
    }

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

    run().await;


}

