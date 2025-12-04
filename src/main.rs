use ffmpeg::codec::traits::Encoder;
use ffmpeg_next as ffmpeg;
use fastrand;


fn intermediate(p1 : [usize; 2], p2 : [usize; 2]) -> [usize; 2] {
    return [(p1[0]+p2[0])/2, (p1[1]+p2[1])/2];
}


fn main() {
    const IMAGE_SIZE : usize = 1500;
    const POINTS : [[usize; 2]; 5] = [[75, 750], [541, 1392], [1296, 1147], [1296, 353], [541, 108]];
    const ITER : usize = 100_000_000;
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
}

