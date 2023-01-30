#version 450

layout(location = 0) in vec2 position;
layout(location = 1) in vec2 tex_position;
layout(location = 0) out vec2 v_tex_position;
layout(location = 1) out vec4 v_color;


void main() {
   
    v_tex_position = tex_position;
    v_color = vec4(1.0, 1.0, 1.0, 1.0);
    gl_Position = vec4(position, 0.0, 1.0);
}