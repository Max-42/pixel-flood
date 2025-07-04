import datetime
from PIL import Image, ImageDraw, ImageFont

def create_transparent_png(width, height, font_path):
    # Create a transparent image
    image = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)

    # Load smaller font
    font = ImageFont.truetype(font_path, 14)

    # Get current time
    current_time = datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")

    # Measure text size
    bbox = draw.textbbox((0, 0), current_time, font=font)
    text_width = bbox[2] - bbox[0]
    text_height = bbox[3] - bbox[1]

    padding = 4
    border_radius = 5
    border_width = 1

    # Calculate box dimensions
    box_width = text_width + 2 * padding
    box_height = text_height + 2 * padding
    box_x0 = width - box_width - 6
    box_y0 = 6
    box_x1 = box_x0 + box_width
    box_y1 = box_y0 + box_height

    # Draw rounded rectangle background with border
    draw.rounded_rectangle(
        [(box_x0, box_y0), (box_x1, box_y1)],
        radius=border_radius,
        fill=(255, 255, 255, 255),
        outline=(128, 0, 128, 255),
        width=border_width
    )

    # Vertical centering adjustment
    text_x = box_x0 + padding
    text_y = box_y0 + (box_height - text_height) // 2

    # Draw the text
    draw.text((text_x, text_y), current_time, fill=(128, 0, 128, 255), font=font)

    return image

if __name__ == "__main__":
    width = 384
    height = 256
    font_path = "Hack-Regular.ttf"  # Make sure the font file is available

    image = create_transparent_png(width, height, font_path)
    image.save("transparent_clock.png")
    print("Transparent PNG created with small, aligned date/time box.")
