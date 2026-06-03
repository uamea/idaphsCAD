fn main() {}

#[cfg(test)]
mod tests {
    use truck_meshalgo::prelude::*;
    use truck_modeling::*;

    #[test]
    fn test_mesh() {
        let vertex = builder::vertex(Point3::new(-1.0, 0.0, -1.0));
        let edge = builder::tsweep(&vertex, 2.0 * Vector3::unit_z());
        let face = builder::tsweep(&edge, 2.0 * Vector3::unit_x());
        let cube = builder::tsweep(&face, 2.0 * Vector3::unit_y());

        let mesh_with_topology = cube.triangulation(0.01);
        
        let a: () = mesh_with_topology; // Cause a type error to see the type and methods
    }
}
