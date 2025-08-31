#version 450
layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 0) uniform Object {
	vec4 color;
} object;

void main() {
    f_color = object.color;
}

