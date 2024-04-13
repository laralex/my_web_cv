struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    var TRI_VERTICES = array(
        vec4(-0.5, -0.5, 0.0, 1.0),
        vec4( 0.0,  0.5, 0.0, 1.0),
        vec4( 0.5, -0.5, 0.0, 1.0),
    );
    var out: VertexOutput;
    out.clip_position = TRI_VERTICES[in_vertex_index];
    return out;
}