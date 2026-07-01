#!/bin/sh

# List of fuzz targets.
# You can comment out individual lines to skip specific targets.
targets=""
targets="$targets fuzz_ct_points"
targets="$targets fuzz_ct_constrained"
targets="$targets fuzz_cdt_points"
targets="$targets fuzz_cdt_constrained"
targets="$targets fuzz_ct_points_delaunay"
targets="$targets fuzz_ct_constrained_delaunay"
targets="$targets fuzz_hex_mesh_cdt"
targets="$targets fuzz_hex_mesh_lattice"

echo "Starting fuzzing session. Each target will run for 30 seconds."

for target in $targets; do
    echo "------------------------------------------------------------"
    echo "Running fuzzer: $target"
    echo "------------------------------------------------------------"

    # Run cargo fuzz for 30 seconds
    # -max_total_time=30 is passed to the underlying libFuzzer
    cargo fuzz run "$target" -- -max_total_time=30

    # Check exit status
    if [ $? -ne 0 ]; then
        echo "Fuzzer $target failed or found a crash! Stopping."
        exit 1
    fi
done

echo "All fuzz targets completed successfully."
