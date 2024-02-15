#![no_std]

use spirv_std::glam::{Vec4, Vec3, Vec2, Mat4};
use spirv_std::Sampler;
use spirv_std::image::Image2d;
use spirv_std::spirv;
use spirv_std::num_traits::Float;

#[spirv(vertex)]
pub fn main_vs(
  pos: Vec3,
  tex_coords: Vec2,
  #[spirv(uniform, descriptor_set = 1, binding = 0)] camera_view_proj: &Mat4,
  #[spirv(uniform, descriptor_set = 2, binding = 0)] time: &f32,
  #[spirv(position)] out_pos: &mut Vec4,
  out_tex_coord: &mut Vec2,
) {
  let y_rot = Mat4::from_rotation_y((core::f32::consts::PI / 45.0) * time * 2.0);

  *out_pos = *camera_view_proj * y_rot * pos.extend(1.0);
  *out_tex_coord = tex_coords;
}

#[spirv(fragment)]
pub fn main_fs(
  #[spirv(descriptor_set = 0, binding = 0)] texture: &Image2d,
  #[spirv(descriptor_set = 0, binding = 1)] sampler: &Sampler,
  in_tex: Vec2,
  output: &mut Vec4,
) {
  *output = texture.sample(*sampler, in_tex);
}
