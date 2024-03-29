struct VertexInput {
  [[location(0)]] position: vec2<f32>;
  [[location(1)]] color: vec3<f32>;
};

struct VertexOutput {
  [[builtin(position)]] position: vec4<f32>;
  [[location(0)]] color: vec3<f32>;
};

struct Offset {
  x: f32;
  y: f32;
};

[[group(0), binding(0)]]
var<uniform> offset: Offset;

[[stage(vertex)]]
fn vs_main(
  model: VertexInput,
) -> VertexOutput {
  var out: VertexOutput;

  let pos = vec2<f32>(
    model.position.x + offset.x,
    model.position.y + offset.y,
  );

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
