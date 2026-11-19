//! Tiles in rows of equal columns, that grow and shrink with the width, as in the Discover
//! tab: two rows of six on a wide screen; then, as the tiles would get narrower than a
//! minimum, two rows of five, three of four, four of three and six of two. The tiles that
//! don't fit are hidden, and are in "See All".

use relm4::gtk::{self, glib, subclass::prelude::*};

/// The columns and rows, from the widest to the narrowest.
const LAYOUTS: [(i32, i32); 5] = [(6, 2), (5, 2), (4, 3), (3, 4), (2, 6)];

glib::wrapper! {
    pub struct TileGrid(ObjectSubclass<imp::TileGrid>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl TileGrid {
    /// A grid whose tiles are from `min_tile_width` to `max_tile_width` wide, which moves to
    /// fewer columns when they would get narrower. Past the widest, the tiles keep to the left
    /// and leave the rest empty. Its tiles must be height-for-width, to grow taller as they
    /// get wider.
    pub fn new(min_tile_width: i32, max_tile_width: i32) -> Self {
        let grid: Self = glib::Object::new();
        grid.imp().tile_widths.set((min_tile_width, max_tile_width));
        grid
    }

    /// Call `f` with whether some tiles are hidden, each time that changes.
    pub fn connect_overflow(&self, f: impl Fn(bool) + 'static) {
        self.imp().on_overflow.replace(Some(Box::new(f)));
    }
}

mod imp {
    use std::cell::{Cell, RefCell};

    use relm4::gtk::{self, glib, prelude::*, subclass::prelude::*};

    use super::LAYOUTS;

    type OverflowHandler = Box<dyn Fn(bool)>;

    /// How the tiles are laid out across a width.
    #[derive(Clone, Copy)]
    struct Layout {
        columns: i32,
        rows: i32,
        /// The width the tiles take, at most that given.
        width: i32,
    }

    impl Layout {
        /// Where `column` starts, and how wide it is: the pixels left over spread across the
        /// columns.
        fn column(self, column: i32) -> (i32, i32) {
            let (x, next_x) = (
                self.width * column / self.columns,
                self.width * (column + 1) / self.columns,
            );
            (x, next_x - x)
        }
    }

    #[derive(Default)]
    pub struct TileGrid {
        /// The narrowest and widest a tile gets.
        pub tile_widths: Cell<(i32, i32)>,
        pub on_overflow: RefCell<Option<OverflowHandler>>,
        /// Whether some tiles were hidden at the last allocation.
        overflowing: Cell<Option<bool>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TileGrid {
        const NAME: &'static str = "PochetteTileGrid";
        type Type = super::TileGrid;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for TileGrid {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for TileGrid {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        /// As narrow as the fewest columns of the narrowest tiles, and as wide as the most
        /// columns of the widest; as tall as its rows at that width.
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let Some(first) = self.obj().first_child() else {
                return (0, 0, -1, -1);
            };
            let widest = LAYOUTS[0].0 * self.tile_widths.get().1;
            if orientation == gtk::Orientation::Horizontal {
                let (tile_minimum, _, _, _) = first.measure(gtk::Orientation::Horizontal, -1);
                let fewest = LAYOUTS[LAYOUTS.len() - 1].0;
                return (tile_minimum * fewest, widest, -1, -1);
            }
            let layout = self.layout(if for_size < 0 { widest } else { for_size });
            let (shown, row_height) = self.rows(layout);
            let rows = (shown + layout.columns - 1) / layout.columns;
            let height = rows * row_height;
            (height, height, -1, -1)
        }

        fn size_allocate(&self, width: i32, _height: i32, _baseline: i32) {
            let layout = self.layout(width);
            let (shown, row_height) = self.rows(layout);
            for (index, child) in children(&self.obj()).enumerate() {
                let index = i32::try_from(index).unwrap_or(i32::MAX);
                let fits = index < shown;
                if child.is_child_visible() != fits {
                    child.set_child_visible(fits);
                }
                if fits {
                    let (column, row) = (index % layout.columns, index / layout.columns);
                    let (x, tile_width) = layout.column(column);
                    child.size_allocate(
                        &gtk::Allocation::new(x, row * row_height, tile_width, row_height),
                        -1,
                    );
                }
            }
            self.report_overflow(shown < self.tile_count());
        }
    }

    impl TileGrid {
        fn tile_count(&self) -> i32 {
            i32::try_from(children(&self.obj()).count()).unwrap_or(i32::MAX)
        }

        /// The most columns that fit `width` with tiles no narrower than the minimum.
        fn layout(&self, width: i32) -> Layout {
            let (min_tile_width, max_tile_width) = self.tile_widths.get();
            let (columns, rows) = LAYOUTS
                .into_iter()
                .find(|&(columns, _)| width / columns >= min_tile_width)
                .unwrap_or(LAYOUTS[LAYOUTS.len() - 1]);
            Layout {
                columns,
                rows,
                width: width.min(columns * max_tile_width),
            }
        }

        /// How many tiles show in `layout`, and how tall its rows are: as the tallest of
        /// those tiles.
        fn rows(&self, layout: Layout) -> (i32, i32) {
            let shown = self.tile_count().min(layout.columns * layout.rows);
            let height = children(&self.obj())
                .take(usize::try_from(shown).unwrap_or(0))
                .zip((0..layout.columns).cycle())
                .map(|(child, column)| {
                    let (_, tile_width) = layout.column(column);
                    child.measure(gtk::Orientation::Vertical, tile_width).1
                })
                .max()
                .unwrap_or(0);
            (shown, height)
        }

        /// Tell [`super::TileGrid::connect_overflow`] whether some tiles are hidden, if that
        /// changed: once the layout is done, as what it does may change sizes.
        fn report_overflow(&self, overflowing: bool) {
            if self.overflowing.replace(Some(overflowing)) == Some(overflowing) {
                return;
            }
            let grid = self.obj().downgrade();
            glib::idle_add_local_once(move || {
                if let Some(grid) = grid.upgrade()
                    && let Some(f) = grid.imp().on_overflow.borrow().as_ref()
                {
                    f(overflowing);
                }
            });
        }
    }

    fn children(grid: &super::TileGrid) -> impl Iterator<Item = gtk::Widget> {
        std::iter::successors(grid.first_child(), gtk::Widget::next_sibling)
    }
}
