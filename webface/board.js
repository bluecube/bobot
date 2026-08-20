const svg_ns = 'http://www.w3.org/2000/svg';

export class Board {
  #board;
  #size;
  #stones_group;
  #move_preview;
  #move_preview_mask;
  #move_preview_last_pos;
  
  constructor(element_id, size) {
    this.#size = Number(size);
    if (!Number.isInteger(this.#size) || this.#size < 2)
      throw new Error("`size` must be integer >= 2, got ${this.#size}");

    this.#board = document.getElementById(element_id);

    this.#board.setAttribute("viewBox", "-1 -1 " + (this.#size + 1) + " " + (this.#size + 1))

    const [children, stones_group, move_preview] = this.#make_board();
    this.#board.replaceChildren(...children);
    this.#stones_group = stones_group;
    this.#move_preview = move_preview;
    this.#move_preview_last_pos = [0, 0];

    this.on_click = null;

    this.#board.addEventListener('pointermove', (event) => {
      const pos = this.#board_coordinates(event);
      if (pos === null || this.check_move_preview_mask(pos))
        this.#hide_move_preview();
      else {
        this.#place_move_preview(pos);
      }
    });
    this.#board.addEventListener('pointerleave', () => this.#hide_move_preview());
    this.#board.addEventListener('pointercancel', () => this.#hide_move_preview());
    this.#board.addEventListener('click', (event) => {
      const pos = this.#board_coordinates(event);
      if (pos !== null && this.on_click) {
        const [row, col] = pos;
        this.on_click(row, col);
      }
    });
  }

  /// Sets the stones to be displayed.
  /// `stones` is an array of strings per row, columns have 'x' representing black 'o' representing
  /// white, any other character is an empty position.
  /// Positions outside the board are placed anyway (but will appear outside the board).
  set_stones(stones) {
    let elements = [];

    for (let r = 0; r < stones.length; r++) {
      const line = stones[r];
      for (let c = 0; c < line.length; c++) {
        if (line[c] === 'x')
          elements.push(this.#make_stone(c, r, 'black'));
        else if (line[c] === 'o')
          elements.push(this.#make_stone(c, r, 'white'));
      }
    }

    this.#stones_group.replaceChildren(...elements);
  }

  /// Move preview can be either null (disabled), 'black', 'white'.
  /// Mask can be either `null` (allowed everywhere) or a list of strings,
  /// one line per row, with character 'x' representing not allowed, any other representing
  /// an allowed position.
  set_move_preview(color, mask = null) {
    if (color !== null && color !== 'black' && color !== 'white')
      throw new Error('move_preview_color must be "black", "white" or null, got ${color}');

    if (mask !== null)
      this.#move_preview_mask = [...mask];
    else
      this.#move_preview_mask = null;

    if (color === null) {
      this.#hide_move_preview();
      return;
    }

    if (this.check_move_preview_mask(this.#move_preview_last_pos)) {
      this.#hide_move_preview();
    }

    this.#move_preview.setAttribute('fill', color);
    this.#move_preview.classList.remove('stone-black');
    this.#move_preview.classList.remove('stone-white');
    this.#move_preview.classList.add('stone-' + color);
  }

  #place_move_preview(pos) {
    const [row, col] = pos;
    this.#move_preview.setAttribute('visibility', 'visible');
    this.#move_preview.setAttribute('cx', col);
    this.#move_preview.setAttribute('cy', row);
    this.#move_preview_last_pos = pos;
  }

  #hide_move_preview() {
    this.#move_preview.setAttribute('visibility', 'hidden');
  }

  /// Returns true if the move preview should be hidden at the given position.
  check_move_preview_mask(pos) {
    const [row, col] = pos;
    return !(this.#move_preview_mask?.[row]?.[col] !== 'x');
  }

  #make_board() {
    let elements = [];

    const lines_group = document.createElementNS(svg_ns, 'g');
    lines_group.classList.add('lines');
    elements.push(lines_group);
    for (let i = 0; i < this.#size; i++) {
      lines_group.appendChild(this.#make_line(i, 0, i, this.#size - 1));
      lines_group.appendChild(this.#make_line(0, i, this.#size - 1, i));
    }

    const stars_group = document.createElementNS(svg_ns, 'g');
    stars_group.classList.add('stars');
    elements.push(stars_group);

    let star_pos1;
    let star_pos2;

    // Star positions for non-standard boards come from this forum post:
    // https://forums.online-go.com/t/star-points-on-non-standard-board-sizes/34723/2
    if (this.#size >= 8) {
      star_pos1 = this.#size < 11 ? 2 : 3;
      star_pos2 = this.#size - star_pos1 - 1;

      stars_group.appendChild(this.#make_star(star_pos1, star_pos1))
      stars_group.appendChild(this.#make_star(star_pos1, star_pos2))
      stars_group.appendChild(this.#make_star(star_pos2, star_pos1))
      stars_group.appendChild(this.#make_star(star_pos2, star_pos2))
    }

    if (this.#size % 2 == 1) {
      const center = Math.floor(this.#size / 2);

      stars_group.appendChild(this.#make_star(center, center));

      if (this.#size >= 17) {
        stars_group.appendChild(this.#make_star(center, star_pos1))
        stars_group.appendChild(this.#make_star(center, star_pos2))
        stars_group.appendChild(this.#make_star(star_pos1, center))
        stars_group.appendChild(this.#make_star(star_pos2, center))
      }
    }

    const move_preview = this.#make_stone(0, 0, 'black'); // The defaults are not important
    move_preview.classList.add('move_preview');
    move_preview.setAttribute('visibility', 'hidden');
    move_preview.setAttribute('opacity', '0.5');
    elements.push(move_preview);

    const stones_group = document.createElementNS(svg_ns, 'g');
    stones_group.classList.add('stones');
    elements.push(stones_group);

    return [elements, stones_group, move_preview];
  }

  #make_line(x1, y1, x2, y2) {
    let l = document.createElementNS(svg_ns, 'line');
    l.setAttribute('x1', x1);
    l.setAttribute('y1', y1);
    l.setAttribute('x2', x2);
    l.setAttribute('y2', y2);
    l.setAttribute('stroke', 'black')
    l.setAttribute('stroke-width', '0.025')
    return l;
  }

  #make_star(x, y) {
    let c = document.createElementNS(svg_ns, 'circle');
    c.setAttribute('cx', x);
    c.setAttribute('cy', y);
    c.setAttribute('r', 0.08);
    c.setAttribute('fill', 'black')
    return c;
  }

  #make_stone(x, y, color) {
    let c = document.createElementNS(svg_ns, 'circle');
    c.classList.add('stone');
    c.classList.add('stone-' + color);
    c.setAttribute('cx', x);
    c.setAttribute('cy', y);
    c.setAttribute('r', 0.3);
    c.setAttribute('fill', color)
    c.setAttribute('stroke', 'black')
    c.setAttribute('stroke-width', '0.025')
    return c;
  }

  #board_coordinates(event) {
    const pt = new DOMPoint(event.clientX, event.clientY).
      matrixTransform(this.#board.getScreenCTM().inverse());
    const row = Math.round(pt.y);
    const col = Math.round(pt.x);

    if (row < 0 || row >= this.#size || col < 0 || col >= this.#size) {
      return null;
    } else {
      return [row, col];
    }
  }
}
