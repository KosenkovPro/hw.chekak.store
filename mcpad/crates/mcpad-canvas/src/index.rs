//! Пространственный индекс canvas.
//!
//! При тысячах узлов проверка каждого узла на пересечение с видимой областью
//! — лишняя работа. Индекс раскладывает узлы по равномерной сетке, и запрос
//! «что видно» смотрит только в ячейки, пересекающиеся с областью.

use std::collections::HashMap;

use crate::layout::Rect;

/// Индекс прямоугольников по равномерной сетке.
#[derive(Debug, Clone)]
pub struct SpatialIndex {
    cell_size: f32,
    cells: HashMap<(i32, i32), Vec<u32>>,
    rects: HashMap<u32, Rect>,
}

impl SpatialIndex {
    /// Индекс с размером ячейки `cell_size` в мировых координатах.
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size: cell_size.max(1.0),
            cells: HashMap::new(),
            rects: HashMap::new(),
        }
    }

    /// Добавляет прямоугольник.
    pub fn insert(&mut self, id: u32, rect: Rect) {
        let (min_x, min_y, max_x, max_y) = self.cell_range(&rect);
        for cell_x in min_x..=max_x {
            for cell_y in min_y..=max_y {
                self.cells.entry((cell_x, cell_y)).or_default().push(id);
            }
        }
        self.rects.insert(id, rect);
    }

    /// Прямоугольник по идентификатору.
    pub fn rect(&self, id: u32) -> Option<Rect> {
        self.rects.get(&id).copied()
    }

    /// Идентификаторы прямоугольников, пересекающихся с областью.
    pub fn query(&self, viewport: &Rect) -> Vec<u32> {
        let (min_x, min_y, max_x, max_y) = self.cell_range(viewport);
        let mut found: Vec<u32> = Vec::new();

        for cell_x in min_x..=max_x {
            for cell_y in min_y..=max_y {
                let Some(ids) = self.cells.get(&(cell_x, cell_y)) else {
                    continue;
                };
                for id in ids {
                    if found.contains(id) {
                        continue;
                    }
                    let Some(rect) = self.rects.get(id) else {
                        continue;
                    };
                    if rect.intersects(viewport) {
                        found.push(*id);
                    }
                }
            }
        }

        found.sort_unstable();
        found
    }

    /// Число проиндексированных прямоугольников.
    pub fn len(&self) -> usize {
        self.rects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rects.is_empty()
    }

    /// Диапазон ячеек, покрывающий прямоугольник.
    fn cell_range(&self, rect: &Rect) -> (i32, i32, i32, i32) {
        let cell = self.cell_size;
        let min_x = (rect.x / cell).floor() as i32;
        let min_y = (rect.y / cell).floor() as i32;
        let max_x = (rect.right() / cell).floor() as i32;
        let max_y = (rect.bottom() / cell).floor() as i32;
        (min_x, min_y, max_x.max(min_x), max_y.max(min_y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_visible_rects() {
        let mut index = SpatialIndex::new(100.0);
        index.insert(
            1,
            Rect {
                x: 0.0,
                y: 0.0,
                width: 50.0,
                height: 50.0,
            },
        );
        index.insert(
            2,
            Rect {
                x: 500.0,
                y: 500.0,
                width: 50.0,
                height: 50.0,
            },
        );
        index.insert(
            3,
            Rect {
                x: 120.0,
                y: 10.0,
                width: 50.0,
                height: 50.0,
            },
        );

        let viewport = Rect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 200.0,
        };
        let found = index.query(&viewport);
        assert!(found.contains(&1));
        assert!(found.contains(&3));
        assert!(!found.contains(&2));
    }

    #[test]
    fn large_rect_is_found_from_any_cell() {
        let mut index = SpatialIndex::new(10.0);
        index.insert(
            1,
            Rect {
                x: 0.0,
                y: 0.0,
                width: 1000.0,
                height: 1000.0,
            },
        );

        let viewport = Rect {
            x: 900.0,
            y: 900.0,
            width: 50.0,
            height: 50.0,
        };
        assert_eq!(index.query(&viewport), vec![1]);
    }

    #[test]
    fn no_duplicates_in_result() {
        let mut index = SpatialIndex::new(20.0);
        index.insert(
            1,
            Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
        );
        let viewport = Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };
        let found = index.query(&viewport);
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn empty_index_returns_nothing() {
        let index = SpatialIndex::new(50.0);
        let viewport = Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };
        assert!(index.query(&viewport).is_empty());
    }
}
