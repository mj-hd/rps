struct VertexInput {
  [[location(0)]] position: vec2<f32>;
  [[location(1)]] color: vec3<f32>;
};

struct VertexOutput {
  [[builtin(position)]] position: vec4<f32>;
  [[location(0)]] color: vec3<f32>;
};

[[stage(vertex)]]
fn vs_main(
  model: VertexInput,
) -> VertexOutput {
  var out: VertexOutput;

  let pos = model.position;

  let x = (pos.x / 512.0) - 1.0;
  let y = 1.0 - (pos.y / 256.0);

  out.position = vec4<f32>(x, y, 0.0, 1.0);
  out.color = model.color;

  return out;
}

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
  return vec4<f32>(in.color, 1.0);
}
