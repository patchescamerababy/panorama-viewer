// Rust sphere mesh generator
// 对应 Java PanoramaViewer3D.buildSphere()

#[derive(Debug, Clone)]
pub struct SphereMesh {
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

pub fn build_sphere(radius: f32, lat: usize, lon: usize) -> SphereMesh {
    let mut positions = Vec::with_capacity((lat + 1) * (lon + 1));
    let mut uvs = Vec::with_capacity((lat + 1) * (lon + 1));
    let mut indices = Vec::new();

    for i in 0..=lat {
        let theta = std::f32::consts::PI * (i as f32) / (lat as f32);
        let y = radius * theta.cos();
        let sin_t = theta.sin();

        for j in 0..=lon {
            let phi = 2.0 * std::f32::consts::PI * (j as f32) / (lon as f32);

            let x = radius * phi.cos() * sin_t;
            let z = radius * phi.sin() * sin_t;

            // JavaFX 版翻转 UV: (1 - u, 1 - v)
            let u = 1.0 - (j as f32) / (lon as f32);
            let v = 1.0 - (i as f32) / (lat as f32);

            positions.push([x, y, z]);
            uvs.push([u, v]);
        }
    }

    for i in 0..lat {
        for j in 0..lon {
            let a = (i * (lon + 1) + j) as u32;
            let b = a + (lon + 1) as u32;

            indices.extend_from_slice(&[
                a, b, a + 1,
                b, b + 1, a + 1,
            ]);
        }
    }

    SphereMesh {
        positions,
        uvs,
        indices,
    }
}
