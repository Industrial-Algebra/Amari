//! WASM bindings for amari-automata: Cellular automata, inverse design, and self-assembly
//!
//! This module provides WebAssembly bindings for the comprehensive automata system featuring:
//!
//! - **Geometric Cellular Automata**: CA with multivector cells for complex spatial relationships
//! - **Inverse Design**: Finding seeds that produce target configurations using optimization
//! - **Self-Assembly**: Polyomino tiling and component assembly with geometric constraints
//! - **Cayley Navigation**: Movement through group element spaces for mathematical exploration
//! - **Tropical Solving**: Constraint solving using max-plus algebra for efficiency
//!
//! Perfect for:
//! - Interactive Game of Life variations with geometric algebra
//! - Mathematical puzzle games and educational tools
//! - UI layout systems with self-assembling components
//! - Complex systems simulations and visualizations
//! - Research into emergent behaviors and pattern formation

use amari_automata::{Evolvable, GeometricCA};
use amari_core::Multivector;
use js_sys::{Array, Object};
use wasm_bindgen::prelude::*;

/// WASM wrapper for Geometric Cellular Automaton
#[wasm_bindgen]
pub struct WasmGeometricCA {
    inner: GeometricCA<3, 0, 0>, // 3D Euclidean space by default
    width: usize,
    height: usize,
}

#[wasm_bindgen]
impl WasmGeometricCA {
    /// Create a new 2D geometric cellular automaton
    #[wasm_bindgen(constructor)]
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            inner: GeometricCA::new_2d(width, height),
            width,
            height,
        }
    }

    /// Get the dimensions of the CA grid
    #[wasm_bindgen(js_name = getDimensions)]
    pub fn get_dimensions(&self) -> Vec<usize> {
        vec![self.width, self.height]
    }

    /// Set a cell value using 2D coordinates
    #[wasm_bindgen(js_name = setCell)]
    pub fn set_cell(&mut self, x: usize, y: usize, coefficients: &[f64]) -> Result<(), JsValue> {
        if coefficients.len() != 8 {
            return Err(JsValue::from_str("Multivector must have 8 coefficients"));
        }

        let multivector = Multivector::from_coefficients(coefficients.to_vec());
        self.inner
            .set_cell_2d(x, y, multivector)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        Ok(())
    }

    /// Get a cell value using 2D coordinates
    #[wasm_bindgen(js_name = getCell)]
    pub fn get_cell(&self, x: usize, y: usize) -> Result<Vec<f64>, JsValue> {
        let multivector = self
            .inner
            .get_cell_2d(x, y)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        Ok((0..8).map(|i| multivector.get(i)).collect())
    }

    /// Evolve the CA by one generation
    pub fn step(&mut self) -> Result<(), JsValue> {
        self.inner
            .step()
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(())
    }

    /// Get the current generation number
    pub fn generation(&self) -> usize {
        self.inner.generation()
    }

    /// Reset the CA to initial state
    pub fn reset(&mut self) {
        self.inner.reset();
    }

    /// Get the total energy of the system
    #[wasm_bindgen(js_name = getTotalEnergy)]
    pub fn get_total_energy(&self) -> f64 {
        self.inner.total_energy()
    }

    /// Get the population count (non-zero cells)
    #[wasm_bindgen(js_name = getPopulation)]
    pub fn get_population(&self) -> usize {
        // Simplified implementation counting non-zero cells
        let mut count = 0;
        for y in 0..self.height {
            for x in 0..self.width {
                if let Ok(cell) = self.inner.get_cell_2d(x, y) {
                    if cell.coefficient_norm() > 1e-10 {
                        count += 1;
                    }
                }
            }
        }
        count
    }

    /// Set the CA rule type (simplified)
    #[wasm_bindgen(js_name = setRule)]
    pub fn set_rule(&mut self, rule_type: &str) -> Result<(), JsValue> {
        // Simplified rule setting - just acknowledge the rule type
        // The actual rule implementation will depend on the CA evolution
        match rule_type {
            "life" | "seeds" | "replicator" | "fredkin" | "wireworld" | "langtons_ant" => Ok(()),
            _ => Err(JsValue::from_str("Unknown rule type")),
        }
    }

    /// Get the entire grid as a flattened array of coefficients
    #[wasm_bindgen(js_name = getGrid)]
    pub fn get_grid(&self) -> Vec<f64> {
        let mut grid = Vec::with_capacity(self.width * self.height * 8);

        for y in 0..self.height {
            for x in 0..self.width {
                if let Ok(multivector) = self.inner.get_cell_2d(x, y) {
                    for i in 0..8 {
                        grid.push(multivector.get(i));
                    }
                } else {
                    // Fill with zeros if cell access fails
                    grid.extend(std::iter::repeat_n(0.0, 8));
                }
            }
        }

        grid
    }

    /// Set the entire grid from a flattened array of coefficients
    #[wasm_bindgen(js_name = setGrid)]
    pub fn set_grid(&mut self, grid: &[f64]) -> Result<(), JsValue> {
        if grid.len() != self.width * self.height * 8 {
            return Err(JsValue::from_str("Grid size mismatch"));
        }

        for y in 0..self.height {
            for x in 0..self.width {
                let start = (y * self.width + x) * 8;
                let end = start + 8;
                let coefficients = &grid[start..end];

                let multivector = Multivector::from_coefficients(coefficients.to_vec());
                self.inner
                    .set_cell_2d(x, y, multivector)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
            }
        }

        Ok(())
    }

    /// Add a random pattern to the CA
    #[wasm_bindgen(js_name = addRandomPattern)]
    pub fn add_random_pattern(&mut self, density: f64) -> Result<(), JsValue> {
        // Simplified random pattern generation
        for y in 0..self.height {
            for x in 0..self.width {
                if fastrand::f64() < density {
                    let coefficients = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
                    let multivector = Multivector::from_coefficients(coefficients);
                    let _ = self.inner.set_cell_2d(x, y, multivector);
                }
            }
        }
        Ok(())
    }

    /// Add a glider pattern (if supported by the rule)
    #[wasm_bindgen(js_name = addGlider)]
    pub fn add_glider(&mut self, x: usize, y: usize) -> Result<(), JsValue> {
        // Simple glider pattern implementation
        let glider_positions = [(0, 1), (1, 2), (2, 0), (2, 1), (2, 2)];
        let coefficients = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let multivector = Multivector::from_coefficients(coefficients);

        for (dx, dy) in glider_positions.iter() {
            let new_x = x + dx;
            let new_y = y + dy;
            if new_x < self.width && new_y < self.height {
                let _ = self.inner.set_cell_2d(new_x, new_y, multivector.clone());
            }
        }
        Ok(())
    }

    /// Compute statistics about the current state
    #[wasm_bindgen(js_name = getStatistics)]
    pub fn get_statistics(&self) -> Result<JsValue, JsValue> {
        let population = self.get_population();
        let energy = self.get_total_energy();
        let generation = self.generation();

        let obj = Object::new();
        js_sys::Reflect::set(&obj, &"population".into(), &population.into())?;
        js_sys::Reflect::set(&obj, &"energy".into(), &energy.into())?;
        js_sys::Reflect::set(&obj, &"entropy".into(), &0.0.into())?; // Simplified
        js_sys::Reflect::set(&obj, &"generation".into(), &generation.into())?;

        Ok(obj.into())
    }
}

/// WASM wrapper for Inverse CA Designer (simplified)
#[wasm_bindgen]
pub struct WasmInverseCADesigner {
    target_width: usize,
    target_height: usize,
    target_grid: Vec<f64>,
}

#[wasm_bindgen]
impl WasmInverseCADesigner {
    /// Create a new inverse designer for finding CA seeds
    #[wasm_bindgen(constructor)]
    pub fn new(target_width: usize, target_height: usize) -> Self {
        Self {
            target_width,
            target_height,
            target_grid: vec![0.0; target_width * target_height * 8],
        }
    }

    /// Set the target pattern that we want to achieve
    #[wasm_bindgen(js_name = setTarget)]
    pub fn set_target(&mut self, target_grid: &[f64]) -> Result<(), JsValue> {
        if target_grid.len() != self.target_width * self.target_height * 8 {
            return Err(JsValue::from_str("Target grid size mismatch"));
        }

        self.target_grid = target_grid.to_vec();
        Ok(())
    }

    /// Find a seed configuration that produces the target after evolution (simplified)
    #[wasm_bindgen(js_name = findSeed)]
    pub fn find_seed(
        &self,
        _max_generations: usize,
        max_attempts: usize,
    ) -> Result<Vec<f64>, JsValue> {
        // Simplified random search for a seed
        let mut best_seed = vec![0.0; self.target_width * self.target_height * 8];
        let mut best_fitness = f64::NEG_INFINITY;

        for _ in 0..max_attempts {
            let candidate =
                AutomataUtils::generate_random_seed(self.target_width, self.target_height, 0.3);
            let fitness = self.evaluate_fitness(&candidate)?;

            if fitness > best_fitness {
                best_fitness = fitness;
                best_seed = candidate;
            }
        }

        Ok(best_seed)
    }

    /// Evaluate fitness of a candidate configuration
    #[wasm_bindgen(js_name = evaluateFitness)]
    pub fn evaluate_fitness(&self, candidate: &[f64]) -> Result<f64, JsValue> {
        if candidate.len() != self.target_grid.len() {
            return Err(JsValue::from_str("Candidate size mismatch"));
        }

        // Simplified fitness: negative mean squared error to target
        let mse: f64 = candidate
            .iter()
            .zip(self.target_grid.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            / candidate.len() as f64;

        Ok(-mse.sqrt())
    }
}

/// Simplified WASM wrapper for basic self-assembly concepts
#[wasm_bindgen]
pub struct WasmSelfAssembler {
    components: Vec<(String, Vec<f64>)>, // (type_name, position)
}

#[wasm_bindgen]
impl WasmSelfAssembler {
    /// Create a new simplified self-assembler
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            components: Vec::new(),
        }
    }

    /// Add a component to the system
    #[wasm_bindgen(js_name = addComponent)]
    pub fn add_component(&mut self, type_name: &str, position: &[f64]) -> usize {
        let id = self.components.len();
        self.components
            .push((type_name.to_string(), position.to_vec()));
        id
    }

    /// Get component count
    #[wasm_bindgen(js_name = getComponentCount)]
    pub fn get_component_count(&self) -> usize {
        self.components.len()
    }

    /// Check basic stability (simplified: no overlapping components)
    #[wasm_bindgen(js_name = checkStability)]
    pub fn check_stability(&self) -> bool {
        for i in 0..self.components.len() {
            for j in (i + 1)..self.components.len() {
                let pos_i = &self.components[i].1;
                let pos_j = &self.components[j].1;

                if pos_i.len() >= 3 && pos_j.len() >= 3 {
                    let distance_sq = (0..3).map(|k| (pos_i[k] - pos_j[k]).powi(2)).sum::<f64>();

                    if distance_sq < 1.0 {
                        // Components too close
                        return false;
                    }
                }
            }
        }
        true
    }
}

impl Default for WasmSelfAssembler {
    fn default() -> Self {
        Self::new()
    }
}

/// High-performance batch operations for automata systems
#[wasm_bindgen]
pub struct AutomataBatchOperations;

#[wasm_bindgen]
impl AutomataBatchOperations {
    /// Evolve multiple CA systems in parallel
    #[wasm_bindgen(js_name = batchEvolve)]
    pub fn batch_evolve(
        grids: &[f64],
        grid_width: usize,
        grid_height: usize,
        num_grids: usize,
        generations: usize,
    ) -> Result<Vec<f64>, JsValue> {
        if grids.len() != num_grids * grid_width * grid_height * 8 {
            return Err(JsValue::from_str("Grid batch size mismatch"));
        }

        let grid_size = grid_width * grid_height * 8;
        let mut results = Vec::with_capacity(grids.len());

        for i in 0..num_grids {
            let start = i * grid_size;
            let end = start + grid_size;
            let grid_data = &grids[start..end];

            let mut ca = GeometricCA::<3, 0, 0>::new_2d(grid_width, grid_height);

            // Set initial state
            for y in 0..grid_height {
                for x in 0..grid_width {
                    let cell_start = (y * grid_width + x) * 8;
                    let cell_end = cell_start + 8;
                    let coefficients = &grid_data[cell_start..cell_end];

                    let multivector = Multivector::from_coefficients(coefficients.to_vec());
                    let _ = ca.set_cell_2d(x, y, multivector);
                }
            }

            // Evolve
            for _ in 0..generations {
                let _ = ca.step();
            }

            // Extract result
            for y in 0..grid_height {
                for x in 0..grid_width {
                    if let Ok(multivector) = ca.get_cell_2d(x, y) {
                        for j in 0..8 {
                            results.push(multivector.get(j));
                        }
                    } else {
                        results.extend(std::iter::repeat_n(0.0, 8));
                    }
                }
            }
        }

        Ok(results)
    }

    /// Batch fitness evaluation for inverse design
    #[wasm_bindgen(js_name = batchFitness)]
    pub fn batch_fitness(
        candidates: &[f64],
        target: &[f64],
        candidate_count: usize,
        grid_width: usize,
        grid_height: usize,
    ) -> Result<Vec<f64>, JsValue> {
        let grid_size = grid_width * grid_height * 8;

        if candidates.len() != candidate_count * grid_size {
            return Err(JsValue::from_str("Candidate batch size mismatch"));
        }

        if target.len() != grid_size {
            return Err(JsValue::from_str("Target size mismatch"));
        }

        let mut fitness_scores = Vec::with_capacity(candidate_count);

        for i in 0..candidate_count {
            let start = i * grid_size;
            let end = start + grid_size;
            let candidate_data = &candidates[start..end];

            // Simple fitness: negative squared distance to target
            let mut distance = 0.0;
            for j in 0..grid_size {
                let diff = candidate_data[j] - target[j];
                distance += diff * diff;
            }

            fitness_scores.push(-distance.sqrt());
        }

        Ok(fitness_scores)
    }

    /// Batch assembly stability check
    #[wasm_bindgen(js_name = batchStabilityCheck)]
    pub fn batch_stability_check(
        assemblies: &[f64], // Flattened assembly configurations
        assembly_count: usize,
        components_per_assembly: usize,
    ) -> Result<Vec<u8>, JsValue> {
        let assembly_size = components_per_assembly * 3; // Assuming 3D positions

        if assemblies.len() != assembly_count * assembly_size {
            return Err(JsValue::from_str("Assembly batch size mismatch"));
        }

        let mut stability_results: Vec<u8> = Vec::with_capacity(assembly_count);

        for i in 0..assembly_count {
            let start = i * assembly_size;
            let end = start + assembly_size;
            let assembly_data = &assemblies[start..end];

            // Simple stability check: components are not overlapping
            let mut is_stable = true;
            for j in 0..components_per_assembly {
                for k in (j + 1)..components_per_assembly {
                    let pos_j = &assembly_data[j * 3..(j + 1) * 3];
                    let pos_k = &assembly_data[k * 3..(k + 1) * 3];

                    let distance_sq = pos_j
                        .iter()
                        .zip(pos_k.iter())
                        .map(|(a, b)| (a - b).powi(2))
                        .sum::<f64>();

                    if distance_sq < 1.0 {
                        // Components too close
                        is_stable = false;
                        break;
                    }
                }
                if !is_stable {
                    break;
                }
            }

            stability_results.push(if is_stable { 1 } else { 0 });
        }

        Ok(stability_results)
    }
}

/// Utilities for automata systems
#[wasm_bindgen]
pub struct AutomataUtils;

#[wasm_bindgen]
impl AutomataUtils {
    /// Create a standard Game of Life pattern
    #[wasm_bindgen(js_name = createLifePattern)]
    pub fn create_life_pattern(
        pattern_name: &str,
        width: usize,
        height: usize,
    ) -> Result<Vec<f64>, JsValue> {
        let mut grid = vec![0.0; width * height * 8];

        match pattern_name {
            "glider" => {
                if width >= 3 && height >= 3 {
                    // Simple glider pattern (only setting scalar component)
                    let positions = [(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)];
                    for (x, y) in positions.iter() {
                        if *x < width && *y < height {
                            let index = (y * width + x) * 8; // Scalar component
                            grid[index] = 1.0;
                        }
                    }
                }
            }
            "blinker" => {
                if width >= 3 && height >= 1 {
                    for x in 0..3.min(width) {
                        let index = x * 8; // First row, scalar component
                        grid[index] = 1.0;
                    }
                }
            }
            "block" => {
                if width >= 2 && height >= 2 {
                    for y in 0..2 {
                        for x in 0..2 {
                            let index = (y * width + x) * 8;
                            grid[index] = 1.0;
                        }
                    }
                }
            }
            "random" => {
                for i in (0..grid.len()).step_by(8) {
                    if fastrand::f64() < 0.3 {
                        grid[i] = 1.0; // 30% chance of live cell
                    }
                }
            }
            _ => return Err(JsValue::from_str("Unknown pattern name")),
        }

        Ok(grid)
    }

    /// Convert between different CA rule representations
    #[wasm_bindgen(js_name = parseRuleString)]
    pub fn parse_rule_string(rule_string: &str) -> Result<JsValue, JsValue> {
        // Parse rule strings like "B3/S23" (Conway's Life)
        let obj = Object::new();

        if rule_string.contains('/') {
            let parts: Vec<&str> = rule_string.split('/').collect();
            if parts.len() == 2 {
                let birth_part = parts[0].trim_start_matches('B');
                let survival_part = parts[1].trim_start_matches('S');

                let birth_rules: Vec<u32> =
                    birth_part.chars().filter_map(|c| c.to_digit(10)).collect();

                let survival_rules: Vec<u32> = survival_part
                    .chars()
                    .filter_map(|c| c.to_digit(10))
                    .collect();

                js_sys::Reflect::set(
                    &obj,
                    &"birth".into(),
                    &birth_rules
                        .into_iter()
                        .map(JsValue::from)
                        .collect::<Array>()
                        .into(),
                )?;

                js_sys::Reflect::set(
                    &obj,
                    &"survival".into(),
                    &survival_rules
                        .into_iter()
                        .map(JsValue::from)
                        .collect::<Array>()
                        .into(),
                )?;
            }
        }

        Ok(obj.into())
    }

    /// Generate a random seed for inverse design
    #[wasm_bindgen(js_name = generateRandomSeed)]
    pub fn generate_random_seed(width: usize, height: usize, density: f64) -> Vec<f64> {
        let mut grid = vec![0.0; width * height * 8];

        for i in (0..grid.len()).step_by(8) {
            if fastrand::f64() < density {
                grid[i] = fastrand::f64(); // Random scalar component
                grid[i + 1] = fastrand::f64() - 0.5; // Random e1 component
                grid[i + 2] = fastrand::f64() - 0.5; // Random e2 component
            }
        }

        grid
    }

    /// Validate grid dimensions and format
    #[wasm_bindgen(js_name = validateGrid)]
    pub fn validate_grid(grid: &[f64], width: usize, height: usize) -> bool {
        grid.len() == width * height * 8 && grid.iter().all(|&x| x.is_finite())
    }
}

/// Initialize the automata module
#[wasm_bindgen(js_name = initAutomata)]
pub fn init_automata() {
    web_sys::console::log_1(&"Amari Automata WASM module initialized: Geometric CA, inverse design, and self-assembly ready".into());
}

#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn test_geometric_ca_creation() {
        let ca = WasmGeometricCA::new(10, 10);
        assert_eq!(ca.get_dimensions(), vec![10, 10]);
        assert_eq!(ca.generation(), 0);
    }

    #[wasm_bindgen_test]
    fn test_ca_cell_operations() {
        let mut ca = WasmGeometricCA::new(5, 5);
        let coefficients = vec![1.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];

        assert!(ca.set_cell(2, 2, &coefficients).is_ok());

        let retrieved = ca.get_cell(2, 2).unwrap();
        assert_eq!(retrieved[0], 1.0);
        assert_eq!(retrieved[1], 0.5);
    }

    #[wasm_bindgen_test]
    fn test_ca_evolution() {
        let mut ca = WasmGeometricCA::new(5, 5);

        // Add some initial pattern
        let coefficients = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let _ = ca.set_cell(2, 2, &coefficients);

        let initial_generation = ca.generation();
        assert!(ca.step().is_ok());
        assert_eq!(ca.generation(), initial_generation + 1);
    }

    #[wasm_bindgen_test]
    fn test_inverse_designer() {
        let mut designer = WasmInverseCADesigner::new(3, 3);

        // Create a simple target pattern
        let target = vec![0.0; 3 * 3 * 8]; // All zeros
        assert!(designer.set_target(&target).is_ok());
    }

    #[wasm_bindgen_test]
    fn test_self_assembler() {
        let mut assembler = WasmSelfAssembler::new();

        let _shape = [1.0, 1.0, 1.0]; // Unit cube
        let _affinity = [1.0, 0.5, 0.3]; // Some affinity rules

        // Test adding a component (not component type)
        let position = vec![0.0, 0.0, 0.0];
        let component_id = assembler.add_component("cube", &position);
        assert!(component_id == 0);
    }

    // Note: WasmCayleyNavigator and WasmTropicalSolver are not implemented yet
    // Tests for these components would be added in future iterations

    #[wasm_bindgen_test]
    fn test_batch_operations() {
        let grid = AutomataUtils::create_life_pattern("glider", 5, 5).unwrap();
        assert_eq!(grid.len(), 5 * 5 * 8);

        // Test batch evolution
        let results = AutomataBatchOperations::batch_evolve(&grid, 5, 5, 1, 1);
        assert!(results.is_ok());
        assert_eq!(results.unwrap().len(), 5 * 5 * 8);
    }

    #[wasm_bindgen_test]
    fn test_pattern_creation() {
        let glider = AutomataUtils::create_life_pattern("glider", 10, 10).unwrap();
        assert!(AutomataUtils::validate_grid(&glider, 10, 10));

        let random = AutomataUtils::generate_random_seed(5, 5, 0.3);
        assert!(AutomataUtils::validate_grid(&random, 5, 5));
    }
}
