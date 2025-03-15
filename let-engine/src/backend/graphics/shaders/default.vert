#version 450

layout (location = 0) in vec2 position;

layout (set = 0, binding = 0) readonly uniform Object {
	mat4 model;
	mat4 view;
	mat4 proj;
} object;

void main() {

    gl_Position = object.proj * object.view * object.model * vec4(position, 0.0, 1.0);

}
