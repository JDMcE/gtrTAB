// Getting a score out of the app as something to look at: the fonts its pictures are drawn
// with, the paper they are sized for, and printing.

export type Paper = 'a4' | 'letter';

/** The paper most people where the user is print on: letter in North America, A4 elsewhere. */
export const paper: Paper = /-(US|CA|MX|PH|CL|CO|VE)\b/i.test(navigator.language) ? 'letter' : 'a4';

/** Width in pixels of the layout that a picture of the whole score is made from. */
export const PICTURE_WIDTH = 1000;

export type Fonts = [music: Uint8Array, text: Uint8Array, fret: Uint8Array];

const FONT_FILES = ['Bravura.otf', 'NotoSans-Regular.ttf', 'NotoSans-SemiBold.ttf'];
let loading: Promise<Fonts> | null = null;

/**
 * The fonts a PDF, a PNG or a printed page is drawn with: the music font and two faces of
 * a text font. They are fetched the first time something is exported, and kept.
 */
export function exportFonts(): Promise<Fonts> {
  loading ??= Promise.all(
    FONT_FILES.map(async (name) => {
      const response = await fetch(`fonts/${name}`);
      if (!response.ok) throw new Error(`The font ${name} could not be loaded.`);
      return new Uint8Array(await response.arrayBuffer());
    }),
  ) as Promise<Fonts>;
  // A failure is not kept: the next export tries again.
  loading.catch(() => (loading = null));
  return loading;
}

/**
 * Prints pages, each an SVG document the size of the paper. They are put in the window
 * beside the app, which a rule for print hides, and taken out again once the print
 * dialog has closed.
 */
export function printPages(pages: string[], size: Paper): Promise<void> {
  const root = document.createElement('div');
  root.className = 'print-root';
  root.innerHTML = pages.join('');
  const style = document.createElement('style');
  style.textContent = `
    @page { size: ${size === 'letter' ? 'letter' : 'A4'}; margin: 0; }
    .print-root { display: none; }
    @media print {
      html, body { margin: 0; background: #fff; }
      body > *:not(.print-root) { display: none !important; }
      .print-root { display: block; }
      .print-root svg { display: block; width: 100%; height: auto; break-after: page; }
      .print-root svg:last-child { break-after: auto; }
    }`;
  document.head.append(style);
  document.body.append(root);
  return new Promise((done) => {
    const finish = () => {
      window.removeEventListener('afterprint', finish);
      root.remove();
      style.remove();
      done();
    };
    window.addEventListener('afterprint', finish);
    window.print();
  });
}
