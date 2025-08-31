#version 450

layout (location = 0) in vec2 position;
layout (location = 1) in vec2 tex_position;
layout (location = 1) out vec2 frag_position;

layout (set = 0, binding = 0) readonly uniform ViewProj {
	mat4 view;
	mat4 proj;
} view_proj;

layout (push_constant) uniform pc {
	mat4 model;
} model;

void main() {
	frag_position = tex_position;

    gl_Position = view_proj.proj * view_proj.view * model.model * vec4(position, 0.0, 1.0);

}
