#!/bin/bash

# Iterate through all .frag files in the current directory
for frag_file in *.frag; do
    # Check if the file exists
    if [ -e "$frag_file" ]; then
        # Extract the file name without extension
        base_name=$(basename -- "$frag_file")
        base_name_no_ext="${base_name%.*}"

        # Compile the fragment shader to SPIR-V
        glslangValidator -V "$frag_file" -o "${base_name_no_ext}_frag.spv"
        
        # Check if the compilation was successful
        if [ $? -eq 0 ]; then
            echo "Successfully compiled $frag_file to ${base_name_no_ext}_frag.spv"
        else
            echo "Failed to compile $frag_file"
        fi
    fi
done

# Iterate through all .vert files in the current directory
for vert_file in *.vert; do
    # Check if the file exists
    if [ -e "$vert_file" ]; then
        # Extract the file name without extension
        base_name=$(basename -- "$vert_file")
        base_name_no_ext="${base_name%.*}"

        # Compile the vertex shader to SPIR-V
        glslangValidator -V "$vert_file" -o "${base_name_no_ext}_vert.spv"
        
        # Check if the compilation was successful
        if [ $? -eq 0 ]; then
            echo "Successfully compiled $vert_file to ${base_name_no_ext}_vert.spv"
        else
            echo "Failed to compile $vert_file"
        fi
    fi
done
