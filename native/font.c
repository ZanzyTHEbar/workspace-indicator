#include <fontconfig/fontconfig.h>
#include <ft2build.h>
#include FT_FREETYPE_H
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

typedef struct {
    unsigned char *pixels;
    int width, height, left, top, advance;
} WiGlyph;

static int generic(const char *family)
{
    static const char *names[] = {
        "serif", "sans-serif", "monospace", "sans", "system-ui",
        "cursive", "fantasy", "emoji", "math"
    };
    for (unsigned int i = 0; i < sizeof(names) / sizeof(names[0]); i++)
        if (!strcasecmp(family, names[i]))
            return 1;
    return 0;
}

static int family_count(FcPattern *pattern, const FcChar8 *family)
{
    int count = 0;
    for (int i = 0; ; i++) {
        FcChar8 *value;
        if (FcPatternGetString(pattern, FC_FAMILY, i, &value) != FcResultMatch)
            return count;
        if (!FcStrCmpIgnoreCase(value, family))
            count++;
    }
}

/* Fontconfig can append even strong default families to every request.
 * An alias must contribute a candidate beyond those unrelated defaults. */
static int requested_family(FcPattern *pattern, FcPattern *defaults,
                            FcPattern *match, const char *family)
{
    if (generic(family))
        return 1;
    for (int i = 0; ; i++) {
        FcChar8 *candidate;
        if (FcPatternGetString(match, FC_FAMILY, i, &candidate) != FcResultMatch)
            return 0;
        if (!FcStrCmpIgnoreCase(candidate, (const FcChar8 *)family) ||
            family_count(pattern, candidate) > family_count(defaults, candidate))
            return 1;
    }
}

static FT_Error installed_face(FT_Library library, const char *family, FT_Face *face)
{
    FT_Error error = 1;
    FcConfig *config = FcConfigCreate();
    FcPattern *pattern = NULL, *defaults = NULL, *match = NULL;
    FcResult result;
    FcChar8 *file;
    int index = 0;
    if (!config || !FcConfigParseAndLoad(config, NULL, FcTrue) ||
        !FcConfigBuildFonts(config))
        goto done;
    pattern = FcPatternCreate();
    if (!pattern || !FcPatternAddString(pattern, FC_FAMILY, (const FcChar8 *)family))
        goto done;
    FcPatternAddInteger(pattern, FC_WEIGHT, FC_WEIGHT_MEDIUM);
    FcPatternAddInteger(pattern, FC_SLANT, FC_SLANT_ROMAN);
    defaults = FcPatternDuplicate(pattern);
    if (!defaults)
        goto done;
    FcPatternDel(defaults, FC_FAMILY);
    FcPatternAddString(defaults, FC_FAMILY,
        (const FcChar8 *)"workspace-indicator-unavailable-family-31c3f4b7");
    if (!FcConfigSubstitute(config, defaults, FcMatchPattern))
        goto done;
    if (!FcConfigSubstitute(config, pattern, FcMatchPattern))
        goto done;
    FcDefaultSubstitute(pattern);
    match = FcFontMatch(config, pattern, &result);
    if (!match || !requested_family(pattern, defaults, match, family) ||
        FcPatternGetString(match, FC_FILE, 0, &file) != FcResultMatch)
        goto done;
    FcPatternGetInteger(match, FC_INDEX, 0, &index);
    error = FT_New_Face(library, (char *)file, index, face);
done:
    if (match)
        FcPatternDestroy(match);
    if (pattern)
        FcPatternDestroy(pattern);
    if (defaults)
        FcPatternDestroy(defaults);
    if (config)
        FcConfigDestroy(config);
    return error;
}

void wi_free_glyphs(WiGlyph *glyphs)
{
    for (int i = 0; i < 10; i++) {
        free(glyphs[i].pixels);
        memset(&glyphs[i], 0, sizeof(glyphs[i]));
    }
}

static int render_digits(FT_Face face, unsigned int size, WiGlyph *glyphs)
{
    if (FT_Select_Charmap(face, FT_ENCODING_UNICODE) ||
        FT_Set_Char_Size(face, 0, size, 72, 72))
        return 0;
    for (int i = 0; i < 10; i++) {
        if (!FT_Get_Char_Index(face, '0' + i) ||
            FT_Load_Char(face, '0' + i,
                         FT_LOAD_RENDER | FT_LOAD_TARGET_NORMAL | FT_LOAD_NO_BITMAP))
            return 0;
        FT_GlyphSlot slot = face->glyph;
        FT_Bitmap *bitmap = &slot->bitmap;
        if (bitmap->pixel_mode != FT_PIXEL_MODE_GRAY ||
            !bitmap->width || !bitmap->rows ||
            bitmap->width > 4096 || bitmap->rows > 4096)
            return 0;
        WiGlyph *glyph = &glyphs[i];
        glyph->width = bitmap->width;
        glyph->height = bitmap->rows;
        glyph->left = slot->bitmap_left;
        glyph->top = slot->bitmap_top;
        glyph->advance = (slot->advance.x + 32) / 64;
        glyph->pixels = malloc((size_t)glyph->width * glyph->height);
        if (!glyph->pixels)
            return 0;
        for (int y = 0; y < glyph->height; y++) {
            int row = bitmap->pitch >= 0 ? y : glyph->height - 1 - y;
            const unsigned char *src = bitmap->buffer + row * abs(bitmap->pitch);
            memcpy(glyph->pixels + (size_t)y * glyph->width, src, glyph->width);
        }
    }
    return 1;
}

/* 0: complete failure, 1: embedded, 2: installed, 3: embedded fallback.
 * This module uses FreeType (FreeType License), Fontconfig (MIT-like), and
 * Expat (MIT); their notices are distributed in native/licenses. */
int wi_prepare_digits(const unsigned char *embedded, long length,
                      const char *family, unsigned int size, WiGlyph *glyphs)
{
    FT_Library library;
    FT_Face face = NULL;
    int selected = 0, success = 0;
    if (FT_Init_FreeType(&library))
        return 0;
    if (family && *family && !installed_face(library, family, &face)) {
        selected = 1;
        success = render_digits(face, size, glyphs);
        FT_Done_Face(face);
    }
    if (!success) {
        wi_free_glyphs(glyphs);
        if (!FT_New_Memory_Face(library, embedded, length, 0, &face)) {
            success = render_digits(face, size, glyphs);
            FT_Done_Face(face);
        }
        selected = 0;
    }
    FT_Done_FreeType(library);
    if (!success) {
        wi_free_glyphs(glyphs);
        return 0;
    }
    return selected ? 2 : (family && *family ? 3 : 1);
}
