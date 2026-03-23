import fitz  # PyMuPDF
import sys

def convert_pdf_to_structured_markdown(file_path, output_path):
    try:
        doc = fitz.open(file_path)
        with open(output_path, 'w', encoding='utf-8') as f:
            for page_num, page in enumerate(doc):
                f.write(f"## Page {page_num + 1}\n\n")
                
                blocks = page.get_text("dict")["blocks"]
                
                for b in blocks:
                    if b['type'] == 0:  # Text block
                        for line in b["lines"]:
                            for span in line["spans"]:
                                text = span["text"].strip()
                                if not text:
                                    continue
                                
                                # Heuristic: larger fonts are treated as headers
                                if span["size"] > 14:
                                    f.write(f"### {text}\n\n")
                                else:
                                    f.write(f"{text}\n")
                        f.write("\n")
                f.write("---\n\n")
                
        print(f"✅ Successfully converted {len(doc)} pages to {output_path}")
    except Exception as e:
        print(f"❌ Error: {e}")

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: python pdf_to_md.py <input.pdf> <output.md>")
    else:
        convert_pdf_to_structured_markdown(sys.argv[1], sys.argv[2])
