#version 460

layout (location = 0) in vec2 position;
layout (location = 1) in vec2 tex_position;
layout (location = 2) in vec4 color;
layout (location = 3) in uint layer;
layout (location = 4) in mat4 model;
layout (location = 8) in mat4 view;
layout (location = 12) in mat4 proj;

layout (location = 1) out vec2 tex_coords;
layout (location = 2) out vec4 frag_color;
layout (location = 3) flat out uint frag_layer;

void main() {

    tex_coords = tex_position;
	frag_color = color;
    frag_layer = layer;

    gl_Position = proj * view * model * vec4(position, 0.0, 1.0);

}
