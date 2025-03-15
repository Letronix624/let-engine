#version 450

layout (location = 0) in vec2 position;
layout (location = 1) in vec2 tex_position;
layout (location = 1) out vec2 tex_coords;

layout (set = 0, binding = 0) readonly uniform Object {
	mat4 model;
	mat4 view;
	mat4 proj;
} object;

void main() {

    tex_coords = tex_position;

    gl_Position = object.proj * object.view * object.model * vec4(position, 0.0, 1.0);

}
