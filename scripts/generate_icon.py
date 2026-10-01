from PIL import Image, ImageDraw
from pathlib import Path

size = 1024
image = Image.new('RGBA', (size, size), (0, 0, 0, 0))
draw = ImageDraw.Draw(image)
draw.rounded_rectangle((24, 24, 1000, 1000), radius=235, fill='#17314e')
draw.rounded_rectangle((176, 177, 848, 847), radius=95, fill='#f5f8f9')
draw.rounded_rectangle((176, 177, 848, 316), radius=95, fill='#78b7a9')
draw.rectangle((176, 256, 848, 316), fill='#78b7a9')
for x in (240, 409, 578):
    draw.rounded_rectangle((x, 377, x + 115, 493), radius=26, fill='#dce7ef')
    draw.rounded_rectangle((x, 546, x + 115, 663), radius=26, fill='#dce7ef')
draw.rounded_rectangle((409, 377, 524, 493), radius=26, fill='#f0b86f')
draw.rounded_rectangle((578, 546, 693, 663), radius=26, fill='#78b7a9')
draw.rounded_rectangle((240, 716, 694, 759), radius=21, fill='#b2cbd5')
path = Path(__file__).resolve().parents[1] / 'src-tauri' / 'icons' / 'icon.png'
path.parent.mkdir(parents=True, exist_ok=True)
image.save(path)
