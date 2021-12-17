use tui::{backend::Backend, layout::Rect, Frame};

pub mod flex_element;
pub use flex_element::FlexElement;

use crate::tuice::{Bounds, Component, DrawContext, Element, Event, LayoutNode, Size, Status};

#[derive(Clone, Copy, Debug)]
pub enum Axis {
    /// Represents the x-axis.
    Horizontal,

    /// Represents the y-axis.
    Vertical,
}

pub struct Flex<'a, Message, B: Backend> {
    children: Vec<FlexElement<'a, Message, B>>,
    alignment: Axis,
}

impl<'a, Message, B: Backend> Flex<'a, Message, B> {
    pub fn new(alignment: Axis) -> Self {
        Self {
            children: vec![],
            alignment,
        }
    }

    /// Creates a new [`Flex`] with a horizontal alignment.
    pub fn row() -> Self {
        Self {
            children: vec![],
            alignment: Axis::Horizontal,
        }
    }

    /// Creates a new [`Flex`] with a horizontal alignment with the given children.
    pub fn row_with_children<C>(children: Vec<C>) -> Self
    where
        C: Into<FlexElement<'a, Message, B>>,
    {
        Self {
            children: children.into_iter().map(Into::into).collect(),
            alignment: Axis::Horizontal,
        }
    }

    /// Creates a new [`Flex`] with a vertical alignment.
    pub fn column() -> Self {
        Self {
            children: vec![],
            alignment: Axis::Vertical,
        }
    }

    /// Creates a new [`Flex`] with a vertical alignment with the given children.
    pub fn column_with_children<C>(children: Vec<C>) -> Self
    where
        C: Into<FlexElement<'a, Message, B>>,
    {
        Self {
            children: children.into_iter().map(Into::into).collect(),
            alignment: Axis::Vertical,
        }
    }

    pub fn with_child<E>(mut self, child: E) -> Self
    where
        E: Into<Element<'a, Message, B>>,
    {
        self.children.push(FlexElement::with_no_flex(child.into()));
        self
    }

    pub fn with_flex_child<E>(mut self, child: E, flex: u16) -> Self
    where
        E: Into<Element<'a, Message, B>>,
    {
        self.children
            .push(FlexElement::with_flex(child.into(), flex));
        self
    }
}

impl<'a, Message, B: Backend> Component<Message, B> for Flex<'a, Message, B> {
    fn draw(&mut self, context: DrawContext<'_>, frame: &mut Frame<'_, B>) {
        self.children
            .iter_mut()
            .zip(context.children())
            .for_each(|(child, child_node)| {
                if child_node.should_draw() {
                    child.draw(child_node, frame);
                }
            });
    }

    fn on_event(&mut self, area: Rect, event: Event, messages: &mut Vec<Message>) -> Status {
        // for child in self.children.iter_mut() {
        //     if let Status::Captured = child.on_event() {
        //         return Status::Captured;
        //     }
        // }

        Status::Ignored
    }

    fn layout(&self, bounds: Bounds, node: &mut LayoutNode) -> Size {
        let mut remaining_bounds = bounds;
        let mut children = vec![LayoutNode::default(); self.children.len()];
        let mut flexible_children_indexes = vec![];
        let mut current_x_offset = 0;
        let mut current_y_offset = 0;
        let mut sizes = Vec::with_capacity(self.children.len());
        let mut current_size = Size::default();
        let mut total_flex = 0;

        // Our general approach is to first handle inflexible children first,
        // then distribute all remaining space to flexible children.
        self.children
            .iter()
            .zip(children.iter_mut())
            .enumerate()
            .for_each(|(index, (child, child_node))| {
                if child.flex == 0 {
                    let size = if remaining_bounds.has_space() {
                        let size = child.child_layout(remaining_bounds, child_node);
                        current_size += size;
                        remaining_bounds.shrink_size(size);

                        size
                    } else {
                        Size::default()
                    };

                    sizes.push(size);
                } else {
                    total_flex += child.flex;
                    flexible_children_indexes.push(index);
                    sizes.push(Size::default());
                }
            });

        flexible_children_indexes.into_iter().for_each(|index| {
            // The index accesses are assumed to be safe by above definitions.
            // This means that we can use the unsafe operations below.
            //
            // NB: If you **EVER** make changes in this function, ensure these assumptions
            // still hold!
            let child = unsafe { self.children.get_unchecked(index) };
            let child_node = unsafe { children.get_unchecked_mut(index) };
            let size = unsafe { sizes.get_unchecked_mut(index) };

            let new_size =
                child.ratio_layout(remaining_bounds, total_flex, child_node, self.alignment);
            current_size += new_size;
            *size = new_size;
        });

        // If there is still remaining space after, distribute the rest if
        // appropriate (e.x. current_size is too small for the bounds).
        if current_size.width < bounds.min_width {
            // For now, we'll cheat and just set it to be equal.
            current_size.width = bounds.min_width;
        }
        if current_size.height < bounds.min_height {
            // For now, we'll cheat and just set it to be equal.
            current_size.height = bounds.min_height;
        }

        // Now that we're done determining sizes, convert all children into the appropriate
        // layout nodes.  Remember - parents determine children, and so, we determine
        // children here!
        sizes
            .iter()
            .zip(children.iter_mut())
            .for_each(|(size, child)| {
                child.rect = Rect::new(current_x_offset, current_y_offset, size.width, size.height);

                match self.alignment {
                    Axis::Horizontal => {
                        current_x_offset += size.width;
                    }
                    Axis::Vertical => {
                        current_y_offset += size.height;
                    }
                }
            });
        node.children = children;

        current_size
    }
}

impl<'a, Message, B: Backend> From<Flex<'a, Message, B>> for Element<'a, Message, B>
where
    Message: 'a,
    B: 'a,
{
    fn from(flex: Flex<'a, Message, B>) -> Self {
        Element::new(flex)
    }
}