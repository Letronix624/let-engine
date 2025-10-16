#version 450
layout (location = 0) out vec4 f_color;

layout (location = 0) in vec3 v_normal;

layout (set = 1, binding = 0) uniform Object {
	vec4 color;
} object;

const vec3 LIGHT = vec3(0.0, 0.0, 1.0);

void main() {
    float brightness = dot(normalize(v_normal), normalize(LIGHT));

    f_color = mix(object.color * 0.6, object.color, brightness);
}

