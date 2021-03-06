use vulkano::device::{Features, Device, DeviceExtensions};
use vulkano::instance::{PhysicalDevice, InstanceExtensions, Instance};
use std::sync::Arc;
use vulkano::image::{StorageImage, Dimensions};
use vulkano::format::Format;
use vulkano::pipeline::ComputePipeline;
use vulkano::descriptor::PipelineLayoutAbstract;
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::buffer::{CpuAccessibleBuffer, BufferUsage};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBuffer};
use vulkano::sync::GpuFuture;
use image::{ImageBuffer, Rgba};
use std::env;
use std::str::FromStr;

fn parse_numeric_argument<T: FromStr>(num: &str) -> T {
    match num.parse::<T>() {
        Ok(num) => num,
        Err(_) => panic!("Could not parse number argument: {}", num)
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    // Get dimension
    let x_dimension: u32 = match args.get(1) {
        None => panic!("X dimension is required."),
        Some(x_dimension) => parse_numeric_argument(&x_dimension)
    };
    let y_dimension: u32 = match args.get(2) {
        None => panic!("Y dimension is required."),
        Some(y_dimension) => parse_numeric_argument(&y_dimension)
    };

    let instance = Instance::new(None, &InstanceExtensions::none(), None)
        .expect("failed to create instance");
    let physical = PhysicalDevice::enumerate(&instance).next().expect("no device available");
    let queue_family = physical.queue_families()
        .find(|&q| q.supports_compute())
        .expect("couldn't find a compute queue family");
    let device_ext = DeviceExtensions {
        khr_storage_buffer_storage_class: true,
        ..DeviceExtensions::none()
    };
    let (device, mut queues) = Device::new(physical, &Features::none(),
                                           &device_ext, [(queue_family, 0.5)].iter().cloned())
        .expect("failed to create device");
    let queue = queues.next().unwrap();

    // Load the shader
    let shader = cs::Shader::load(device.clone()).expect("failed to create shader module");

    let compute_pipeline = Arc::new(
        ComputePipeline::new(device.clone(), &shader.main_entry_point(), &(), None)
            .expect("failed to create compute pipeline")
    );

    // Create an image
    let image = StorageImage::new(device.clone(), Dimensions::Dim2d { width: x_dimension, height: y_dimension },
                                  Format::R8G8B8A8Unorm, Some(queue.family())).unwrap();

    let layout = compute_pipeline.layout().descriptor_set_layout(0).unwrap();
    let set = Arc::new(
        PersistentDescriptorSet::start(layout.clone())
            .add_image(image.clone()).unwrap()
            .build().unwrap()
    );

    let buf = CpuAccessibleBuffer::from_iter(
        device.clone(),
        BufferUsage::all(),
        false,
        (0..x_dimension * y_dimension * 4).map(|_| 0u8)
    ).expect("failed to create buffer");

    // Create command buffer with a draw command dispatch followed by a copy image to buffer command
    let mut builder = AutoCommandBufferBuilder::new(device.clone(), queue.family()).unwrap();
    builder.dispatch([x_dimension / 8, y_dimension / 8, 1], compute_pipeline.clone(), set.clone(), ()).unwrap()
        .copy_image_to_buffer(image.clone(), buf.clone()).unwrap();
    let command_buffer = builder.build().unwrap();

    // Execute draw command and save resulting image
    let finished = command_buffer.execute(queue.clone()).unwrap();
    finished.then_signal_fence_and_flush().unwrap().wait(None).unwrap();

    let buffer_content = buf.read().unwrap();
    let image = ImageBuffer::<Rgba<u8>, _>::from_raw(x_dimension, y_dimension, &buffer_content[..]).unwrap();
    image.save("image.png").unwrap();
}

mod cs {
    vulkano_shaders::shader! {
        ty: "compute",
        path: "src/mandelbrot.comp"
    }
}
