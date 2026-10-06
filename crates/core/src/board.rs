//! Secret board and public observations are distinct types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CellId(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoardSpec {
    pub width: u8,
    pub height: u8,
    pub mines: u16,
    pub opening: CellId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoardError {
    InvalidDimensions,
    InvalidMineCount,
    InvalidCell,
    DuplicateMine,
    ObservationMismatch,
}

impl BoardSpec {
    pub fn validate(self) -> Result<Self, BoardError> {
        if self.width == 0 || self.height == 0 || self.area() > 256 {
            return Err(BoardError::InvalidDimensions);
        }
        if usize::from(self.mines) >= self.area() {
            return Err(BoardError::InvalidMineCount);
        }
        if usize::from(self.opening.0) >= self.area() {
            return Err(BoardError::InvalidCell);
        }
        Ok(self)
    }

    pub fn area(self) -> usize {
        usize::from(self.width) * usize::from(self.height)
    }
    pub fn contains(self, cell: CellId) -> bool {
        usize::from(cell.0) < self.area()
    }
    pub fn neighbors(self, cell: CellId) -> Vec<CellId> {
        if !self.contains(cell) || self.width == 0 {
            return Vec::new();
        }
        let x = i32::from(cell.0) % i32::from(self.width);
        let y = i32::from(cell.0) / i32::from(self.width);
        let mut cells = Vec::with_capacity(8);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (nx, ny) = (x + dx, y + dy);
                if (dx != 0 || dy != 0)
                    && nx >= 0
                    && nx < i32::from(self.width)
                    && ny >= 0
                    && ny < i32::from(self.height)
                {
                    cells.push(CellId((ny * i32::from(self.width) + nx) as u16));
                }
            }
        }
        cells
    }
}

impl Default for BoardSpec {
    fn default() -> Self {
        Self {
            width: 16,
            height: 16,
            mines: 40,
            opening: CellId(0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Mine,
    Number(u8),
}

// Deliberately has no Debug/Serialize implementation.
#[derive(Clone, PartialEq, Eq)]
pub struct Board {
    spec: BoardSpec,
    cells: Vec<Cell>,
}

impl Board {
    pub fn from_mines(spec: BoardSpec, mines: &[CellId]) -> Result<Self, BoardError> {
        let spec = spec.validate()?;
        if mines.len() != usize::from(spec.mines) {
            return Err(BoardError::InvalidMineCount);
        }
        let mut cells = vec![Cell::Number(0); spec.area()];
        for &mine in mines {
            if !spec.contains(mine) {
                return Err(BoardError::InvalidCell);
            }
            if cells[usize::from(mine.0)] == Cell::Mine {
                return Err(BoardError::DuplicateMine);
            }
            cells[usize::from(mine.0)] = Cell::Mine;
        }
        for index in 0..spec.area() {
            if cells[index] != Cell::Mine {
                let adjacent = spec
                    .neighbors(CellId(index as u16))
                    .into_iter()
                    .filter(|cell| cells[usize::from(cell.0)] == Cell::Mine)
                    .count();
                cells[index] = Cell::Number(adjacent as u8);
            }
        }
        Ok(Self { spec, cells })
    }
    pub fn spec(&self) -> BoardSpec {
        self.spec
    }
    pub fn cell(&self, cell: CellId) -> Option<Cell> {
        self.cells.get(usize::from(cell.0)).copied()
    }
    pub fn safe_total(&self) -> usize {
        self.spec.area() - usize::from(self.spec.mines)
    }

    pub fn reveal(
        &self,
        view: &mut Observation,
        target: CellId,
        flags: &[bool],
    ) -> Result<Vec<CellId>, BoardError> {
        if view.spec != self.spec || flags.len() != self.spec.area() {
            return Err(BoardError::ObservationMismatch);
        }
        if !self.spec.contains(target) {
            return Err(BoardError::InvalidCell);
        }
        let mut pending = vec![target];
        let mut opened = Vec::new();
        while let Some(cell) = pending.pop() {
            let index = usize::from(cell.0);
            if flags[index] || view.cells[index] != ObservedCell::Unknown {
                continue;
            }
            match self.cells[index] {
                Cell::Mine => view.cells[index] = ObservedCell::Mine,
                Cell::Number(value) => {
                    view.cells[index] = ObservedCell::Number(value);
                    opened.push(cell);
                    if value == 0 {
                        pending.extend(self.spec.neighbors(cell));
                    }
                }
            }
        }
        opened.sort_unstable();
        Ok(opened)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservedCell {
    Unknown,
    Number(u8),
    Mine,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    spec: BoardSpec,
    cells: Vec<ObservedCell>,
}

impl Observation {
    pub fn new(spec: BoardSpec, cells: Vec<ObservedCell>) -> Result<Self, BoardError> {
        let spec = spec.validate()?;
        if cells.len() != spec.area()
            || cells.iter().any(|c| matches!(c, ObservedCell::Number(9..)))
        {
            return Err(BoardError::ObservationMismatch);
        }
        Ok(Self { spec, cells })
    }
    pub fn closed(spec: BoardSpec) -> Result<Self, BoardError> {
        Self::new(spec, vec![ObservedCell::Unknown; spec.validate()?.area()])
    }
    pub fn spec(&self) -> BoardSpec {
        self.spec
    }
    pub fn cells(&self) -> &[ObservedCell] {
        &self.cells
    }
    pub fn cell(&self, cell: CellId) -> Option<ObservedCell> {
        self.cells.get(usize::from(cell.0)).copied()
    }
    pub fn set(&mut self, cell: CellId, value: ObservedCell) -> Result<(), BoardError> {
        if matches!(value, ObservedCell::Number(9..)) {
            return Err(BoardError::ObservationMismatch);
        }
        *self
            .cells
            .get_mut(usize::from(cell.0))
            .ok_or(BoardError::InvalidCell)? = value;
        Ok(())
    }
    pub fn opened_safe(&self) -> usize {
        self.cells
            .iter()
            .filter(|c| matches!(c, ObservedCell::Number(_)))
            .count()
    }
}
