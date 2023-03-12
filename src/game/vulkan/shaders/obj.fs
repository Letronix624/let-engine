#version 450
layout (location = 0) out vec4 f_color;
layout (location = 1) in vec2 tex_coords;
layout (location = 2) in vec4 vertex_color;
layout (location = 3) flat in uint textureID;
layout (location = 4) flat in uint material;
layout (set = 0, binding = 0) uniform sampler2DArray tex;

void main() {
    vec4 color = vertex_color;
    if (material == 1) {
        color = texture(tex, vec3(tex_coords * 0.5 + 0.5, textureID));
    }
    else if (material == 2) { //grayscale transparency
        color = vec4(vertex_color.rgb, texture(tex, vec3(tex_coords, textureID)).r);
    }
    f_color = color;// * 2.2;
}

