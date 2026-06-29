use std::collections::HashMap;
use std::path::Path;
use truck_meshalgo::prelude::*;
use truck_modeling::{EdgeID, FaceID, Solid, VertexID};
use truck_polymesh::PolygonMesh;

#[derive(Default, Clone, Debug)]
pub struct SelectionState {
    pub selected_faces: Vec<FaceID>,
    pub selected_edges: Vec<EdgeID>,
    pub selected_vertices: Vec<VertexID>,
}

pub struct CadData {
    pub topology: Solid,
    pub face_meshes: HashMap<FaceID, PolygonMesh>,
    pub edge_meshes: HashMap<EdgeID, Vec<Point3>>,
    pub vertex_meshes: HashMap<VertexID, Point3>,
    pub selection: SelectionState,
}

impl CadData {
    pub fn new() -> Self {
        let solid = Solid::new(vec![]);

        CadData::from_solid(solid)
    }

    pub fn from_solid(solid: Solid) -> Self {
        let mut face_meshes = HashMap::new();
        let mut edge_meshes = HashMap::new();
        let mut vertex_meshes = HashMap::new();

        for face in solid.face_iter() {
            let shell = truck_modeling::Shell::from(vec![face.clone()]);
            let mesh = shell.triangulation(0.01).to_polygon();
            face_meshes.insert(face.id(), mesh);
        }

        // Extract edge lines
        for edge in solid.edge_iter() {
            let curve = edge.curve();
            // Just saving endpoints for now as it's a straight line, but if it's a curve, we'd need more points.
            let p0 = curve.front();
            let p1 = curve.back();
            edge_meshes.insert(edge.id(), vec![p0, p1]);
        }

        // Extract vertices
        for vertex in solid.vertex_iter() {
            vertex_meshes.insert(vertex.id(), vertex.point());
        }

        Self {
            topology: solid,
            face_meshes,
            edge_meshes,
            vertex_meshes,
            selection: SelectionState::default(),
        }
    }
}

impl Default for CadData {
    fn default() -> Self {
        Self::new()
    }
}
