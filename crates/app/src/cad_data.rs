use truck_modeling::Solid;
use truck_polymesh::PolygonMesh;
pub struct CadData {
    topology: Solid,
    mesh_data: PolygonMesh,
    selection: SelectionState,
}

use truck_modeling::{EdgeID, FaceID, VertexID};

#[derive(Default)]
pub struct SelectionState {
    pub selected_faces: Vec<FaceID>,
    pub selected_edges: Vec<EdgeID>,
    pub selected_vertices: Vec<VertexID>,
}

pub struct FaceSubMesh {
    pub face_id: FaceID,
    pub index_start: u32,
    pub index_count: u32,
}
