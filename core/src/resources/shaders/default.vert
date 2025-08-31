#version 450

layout (location = 0) in vec2 position;

layout (set = 0, binding = 0) readonly uniform ViewProj {
	mat4 view;
	mat4 proj;
} view_proj;

layout (push_constant) uniform pc {
	mat4 model;
} model;

void main() {

    gl_Position = view_proj.proj * view_proj.view * model.model * vec4(position, 0.0, 1.0);

}
