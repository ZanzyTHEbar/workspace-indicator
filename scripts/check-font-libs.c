#include <fontconfig/fontconfig.h>
#include <ft2build.h>
#include FT_FREETYPE_H
#include <stdio.h>

/* Native-boundary smoke check: link and rasterize real font data, without tools. */
int main(int argc, char **argv)
{
    FT_Library library;
    FT_Face face;
    FcConfig *config;
    FcPattern *pattern, *match;
    FcResult result;
    FcChar8 *file;
    unsigned long coverage = 0;
    if (argc != 2 || FT_Init_FreeType(&library) ||
        FT_New_Face(library, argv[1], 0, &face) ||
        FT_Set_Char_Size(face, 0, 14 * 64, 72, 72))
        return 1;
    for (unsigned int digit = '0'; digit <= '9'; digit++) {
        if (!FT_Get_Char_Index(face, digit) ||
            FT_Load_Char(face, digit, FT_LOAD_RENDER | FT_LOAD_TARGET_NORMAL))
            return 2;
        FT_Bitmap *bitmap = &face->glyph->bitmap;
        for (unsigned int y = 0; y < bitmap->rows; y++)
            for (unsigned int x = 0; x < bitmap->width; x++)
                coverage += bitmap->buffer[y * bitmap->pitch + x];
    }
    if (!coverage)
        return 3;
    config = FcInitLoadConfigAndFonts();
    if (!config)
        return 4;
    pattern = FcPatternCreate();
    if (!pattern)
        return 5;
    FcPatternAddString(pattern, FC_FAMILY, (const FcChar8 *)"sans-serif");
    FcConfigSubstitute(config, pattern, FcMatchPattern);
    FcDefaultSubstitute(pattern);
    match = FcFontMatch(config, pattern, &result);
    if (!match || FcPatternGetString(match, FC_FILE, 0, &file) != FcResultMatch)
        return 6;
    printf("Fontconfig %d, FreeType digits coverage %lu, sans-serif %s\n",
           FcGetVersion(), coverage, file);
    FcPatternDestroy(match);
    FcPatternDestroy(pattern);
    FcConfigDestroy(config);
    FT_Done_Face(face);
    FT_Done_FreeType(library);
    return 0;
}
