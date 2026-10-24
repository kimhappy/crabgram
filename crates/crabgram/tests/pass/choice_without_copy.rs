#[derive(crabgram::Choice)]
enum Shape {
    Sine,
    Square,
}

fn main() {
    let shape = Shape::Square;
    let copy = shape;

    assert_eq!(shape.index(), copy.index());
}
