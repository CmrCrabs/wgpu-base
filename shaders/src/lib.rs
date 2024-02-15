#![no_std]

use spirv_std::glam::{Vec4, Vec3, Vec2, Mat4};
use spirv_std::Sampler;
use spirv_std::image::Image2d;
use spirv_std::spirv;
use spirv_std::num_traits::Float;

const PHI: f32 = 1.61803398874989484820459;
const DENSITY: f32 = 1000.0;
const SHELLS: f32 = 32.0;
const GAP: f32 = 0.01;
const THICKNESS: f32 = 5.0;

#[spirv(vertex)]
pub fn main_vs(
  pos: Vec3,
  tex_coords: Vec2,
  normal: Vec3,
  #[spirv(instance_index)] instance_index: u32,
  #[spirv(uniform, descriptor_set = 1, binding = 0)] camera_view_proj: &Mat4,
  #[spirv(uniform, descriptor_set = 2, binding = 0)] time: &f32,
  #[spirv(position)] out_pos: &mut Vec4,
  out_tex_coord: &mut Vec2,
  out_instance_index: &mut u32,
) {
  let y_rot = Mat4::from_rotation_y((core::f32::consts::PI / 45.0) * time * 2.0);
  let scale = Mat4::from_scale(Vec3::new(2.0, 2.0, 2.0));

  let shell_pos = pos + (normal * GAP) * instance_index as f32;

  *out_pos = *camera_view_proj * y_rot * shell_pos.extend(1.0);
  *out_tex_coord = tex_coords;
  *out_instance_index = instance_index;
}

#[spirv(fragment)]
pub fn main_fs(
  #[spirv(descriptor_set = 0, binding = 0)] texture: &Image2d,
  #[spirv(descriptor_set = 0, binding = 1)] sampler: &Sampler,
  in_tex: Vec2,
  #[spirv(flat)] instance_index: u32,
  output: &mut Vec4,
) {
  let local_tex = (in_tex * DENSITY).fract() * 2.0 - 1.0;
  let dist = local_tex.length();

  let noise = hash((in_tex * DENSITY).trunc());
  let h = instance_index as f32 / SHELLS;

  if instance_index > 0 && dist > THICKNESS * (noise - h) {
    spirv_std::arch::kill();
  }

  let color = texture.sample(*sampler, in_tex);
  //let color = Vec3::new(
  //  gold_noise(local_tex, 1.0),
  // gold_noise(local_tex, 1.5),
  // gold_noise(local_tex, 2.0),
  //)
  //.extend(1.0);
  let ao = (instance_index as f32 / SHELLS + 0.3).min(1.0);
  *output = color * ao;
}

fn gold_noise(xy: Vec2, seed: f32) -> f32 {
  ((xy.distance(xy * PHI) * seed).tan() * xy.x).fract()
}
fn hash(x: Vec2) -> f32 {
  let x = (1.0 / 4320.0) * x + Vec2::new(0.25, 0.0);
  let state = (x * x).dot(Vec2::splat(3571.0)).fract();
  (state * state * 3571.0 * 2.0).fract()
}
