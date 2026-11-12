"""Gera ui/icons.slint: ícones rasterizados como retângulos de 1 px com alfa.

O renderizador de software do Slint (sem a feature software-renderer-path)
não desenha Path nem SVG. Os ícones de traço (geometria Lucide, ISC) viram
varreduras de retângulos. Rode: python3 tools/gen_icons.py > ui/icons.slint
"""
import math

SS = 4  # subamostras por eixo


def seg_dist(px, py, ax, ay, bx, by):
    dx, dy = bx - ax, by - ay
    l2 = dx * dx + dy * dy
    t = 0 if l2 == 0 else max(0, min(1, ((px - ax) * dx + (py - ay) * dy) / l2))
    return math.hypot(px - (ax + t * dx), py - (ay + t * dy))


def inside_poly(px, py, poly):
    c = False
    n = len(poly)
    for i in range(n):
        x1, y1 = poly[i]
        x2, y2 = poly[(i + 1) % n]
        if (y1 > py) != (y2 > py) and px < (x2 - x1) * (py - y1) / (y2 - y1) + x1:
            c = not c
    return c


def raster(w, h, test):
    cov = {}
    for y in range(h):
        for x in range(w):
            n = 0
            for sy in range(SS):
                for sx in range(SS):
                    if test(x + (sx + .5) / SS, y + (sy + .5) / SS):
                        n += 1
            a = round(n / (SS * SS) * 16) / 16
            if a > 0:
                cov[(x, y)] = a
    return cov


def emit(cov, w, h, color_expr, indent="        ", extra=""):
    out = []
    for y in range(h):
        x = 0
        while x < w:
            a = cov.get((x, y), 0)
            if a == 0:
                x += 1
                continue
            x2 = x
            while x2 + 1 < w and cov.get((x2 + 1, y), 0) == a:
                x2 += 1
            n = x2 - x + 1
            bg = color_expr if a == 1 else f"{color_expr}.with-alpha({a:g})"
            out.append(f"{indent}Rectangle {{ x: {x}px; y: {y}px; width: {n}px; height: 1px; background: {bg}; }}")
            x = x2 + 1
    return "\n".join(out)


def polyline_test(pts, width, scale):
    pts = [(x * scale, y * scale) for x, y in pts]
    r = width * scale / 2
    return lambda px, py: any(seg_dist(px, py, *pts[i], *pts[i + 1]) <= r for i in range(len(pts) - 1))


print("// Gerado por tools/gen_icons.py. Não edite à mão.")
print("// Ícones como varredura de retângulos: o renderizador de software não desenha Path/SVG.\n")

# Bluetooth 28x28 (viewBox 24, traço 1.8)
S = 28 / 24
bt = [(7, 7), (17, 17), (12, 22), (12, 2), (17, 7), (7, 17)]
cov = raster(28, 28, polyline_test(bt, 1.8, S))
print("export component IconBluetooth inherits Rectangle {\n    in property <color> color: #ffffff;\n    width: 28px; height: 28px;")
print(emit(cov, 28, 28, "root.color"))
print("}\n")

# Raio 13x13: polígono preenchido + traço 1.8 arredondado
S = 13 / 24
bolt = [(13, 2), (3, 14), (12, 14), (11, 22), (21, 10), (12, 10)]
bp = [(x * S, y * S) for x, y in bolt]
r = 1.8 * S / 2
def bolt_test(px, py):
    if inside_poly(px, py, bp):
        return True
    return any(seg_dist(px, py, *bp[i], *bp[(i + 1) % len(bp)]) <= r for i in range(len(bp)))
cov = raster(13, 13, bolt_test)
print("export component IconBolt inherits Rectangle {\n    in property <color> color: #ffffff;\n    width: 13px; height: 13px;")
print(emit(cov, 13, 13, "root.color"))
print("}\n")

# Traço de "desligado" do emblema 64x64: linha de 44 px a 45°, centro (32,32).
c = 32.0
half = 22.0
k = math.sqrt(0.5)
a = (c - half * k, c - half * k)
b = (c + half * k, c + half * k)
def slash_test(width, cap):
    r = width / 2
    return lambda px, py: seg_dist(px, py, *a, *b) <= r
cov_o = raster(64, 64, slash_test(6, 3))
cov_r = raster(64, 64, slash_test(2, 1))
print("// Traço diagonal do emblema (64x64). O contorno separa o traço do símbolo.")
print("export component BtSlash inherits Rectangle {\n    in property <color> outline: #000000;\n    in property <color> color: #ff0000;\n    width: 64px; height: 64px;")
print(emit(cov_o, 64, 64, "root.outline"))
print(emit(cov_r, 64, 64, "root.color"))
print("}")
