"""Renders the Agent Window mascot: the official Axolotl Launcher axolotl,
posed with the launcher's sticker toolkit and saved as horizontal sprite
strips that src/mascot.css plays with steps().

    python packages/agent-window/dev/render-mascot.py <toolkit> [--out DIR] [--preview DIR]

<toolkit> is the 美西螈表情包工具包 folder (it holds src/rig2.py,
src/rest_pose.json, src/camera.json and model/axolotl.gltf). Only numpy and
Pillow are needed. Strips go to src/mascot/ next to this script unless --out
says otherwise; --preview also writes contact sheets and GIFs for review.

Every frame of every mood shares one camera, scale and centre, measured from
the union of all poses, so nothing jumps between frames or moods and the hop
and gill flares never clip. If you change a motion or a frame count here,
mirror the frame count and duration in src/mascot.css.
"""

import argparse
import copy
import json
import math
import os
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

TAU = 2 * math.pi
HERO = 192  # 2x the 96 px hero
SMALL = 56  # 2x the 28 px status-line mark
SUPERSAMPLE = 4
MARGIN = 0.045  # empty border around the union of all poses, per side
# A soft contact shadow on the ground under the head and body. It grounds the
# mark on light backgrounds (it vanishes on dark ones) and shrinks and fades
# during the hop so the jump reads. Radii in model units, colour a deep plum.
SHADOW = {'rx': 0.36, 'rz': 0.48, 'cz': -0.14, 'alpha': 0.24, 'blur': 0.03, 'rgb': (40, 8, 30)}
HOP_MAX = 3.6


# --- easing -----------------------------------------------------------------

def clamp(x, lo=0.0, hi=1.0):
    return lo if x < lo else hi if x > hi else x


def smooth(x):
    x = clamp(x)
    return x * x * (3 - 2 * x)


def smoother(x):
    x = clamp(x)
    return x * x * x * (x * (6 * x - 15) + 10)


def window(t, a, b):
    """0 before a, 1 after b, smooth in between."""
    return smoother((t - a) / (b - a))


def wave(phase, k=1, offset=0.0):
    return math.sin(TAU * (k * phase + offset))


def pulse(t, a, b):
    """Smooth 0 -> 1 -> 0 bump over [a, b]."""
    if t <= a or t >= b:
        return 0.0
    return math.sin(math.pi * (t - a) / (b - a)) ** 2


# --- pose building ------------------------------------------------------------

class Pose:
    """Offsets on top of the toolkit's rest pose. Angles in degrees, moves in
    model pixels. `lift` raises everything but the legs (breathing, crouch);
    `hop` raises the whole animal; `lid` closes the eyes (0 open, 1 shut)."""

    def __init__(self):
        self.rot = {}
        self.lift = 0.0
        self.hop = 0.0
        self.pitch = 0.0
        self.lid = 0.0

    def r(self, bone, x=0.0, y=0.0, z=0.0):
        a = self.rot.setdefault(bone, [0.0, 0.0, 0.0])
        a[0] += x
        a[1] += y
        a[2] += z
        return self

    def gills(self, flare=0.0, sweep=0.0, top=0.0):
        """flare > 0 lifts the side gills, < 0 droops them; sweep > 0 folds
        them back along the head; top tips the crown gills forward."""
        self.r('left_gills', y=sweep, z=flare)
        self.r('right_gills', y=-sweep, z=-flare)
        self.r('top_gills', x=top)
        return self

    def build(self, rest):
        pose = copy.deepcopy(rest)
        for bone, (x, y, z) in self.rot.items():
            entry = pose.setdefault(bone, {})
            base = entry.get('rot', [0, 0, 0])
            entry['rot'] = [base[0] + x, base[1] + y, base[2] + z]
        # The gills ride on the head bone, so lifting the head lifts them too.
        for bone in ('head', 'body', 'tail') if self.lift else ():
            entry = pose.setdefault(bone, {})
            x, y, z = entry.get('move', [0, 0, 0])
            entry['move'] = [x, y + self.lift, z]
        if self.hop or self.pitch:
            pose['root'] = {'rot': [self.pitch, 0, 0], 'move': [0, self.hop, 0]}
        return pose


# --- moods --------------------------------------------------------------------
# Loops are sampled at phase i / n so the last frame flows into the first.
# One-shots are sampled at i / (n - 1) so the last frame is the final pose.

def idle(p):
    """2.4 s: one slow breath, gills and tail drifting around it, and a blink
    late in the exhale. Frame 0 sits almost exactly on the rest pose."""
    o = Pose()
    breath = 0.5 - 0.5 * math.cos(TAU * p)
    o.lift = 0.4 * breath
    o.r('head', x=1.8 * (0.5 - 0.5 * math.cos(TAU * (p - 0.05))), y=2.0 * wave(p), z=1.0 * wave(p, 1, 0.03))
    o.r('left_gills', y=3.0 * wave(p, 1, 0.05), z=7.0 * wave(p, 1, -0.03) + 1.5 * wave(p, 2, 0.02))
    o.r('right_gills', y=-3.0 * wave(p), z=-(7.0 * wave(p, 1, 0.03) + 1.5 * wave(p, 2, 0.06)))
    o.r('top_gills', x=6.0 * wave(p, 1, -0.06) + 1.2 * wave(p, 2))
    o.r('tail', x=-2.0 * breath, y=swing(7.0 * wave(p, 1, -0.04)))
    o.lid = blink(p, 0.78, 0.115)
    return o


def swing(y):
    """Tail yaw: a positive yaw turns the tail towards the camera, where it
    foreshortens into a stick, so that side gets half the travel."""
    return y * 0.5 if y > 0 else y


def blink(p, at, length):
    x = (p - at) / length
    if x <= 0 or x >= 1:
        return 0.0
    # Close fast, hold a beat, open a little slower.
    if x < 0.35:
        return smooth(x / 0.35)
    if x < 0.5:
        return 1.0
    return 1 - smooth((x - 0.5) / 0.5)


def think(p):
    """1.2 s: a curious side-to-side head tilt, a small lift each way, gills
    fluttering three times, tail following the head."""
    o = Pose()
    lean = wave(p, 1)
    lift = 0.5 - 0.5 * math.cos(TAU * 2 * p)
    o.lift = 0.22 * lift
    o.r('head', x=3.0 * lift, y=5.0 * lean, z=7.5 * wave(p, 1, -0.04))
    flutter = [wave(p, 3, -0.04 * k) for k in range(3)]
    o.r('left_gills', y=-3.0 * flutter[0], z=7.0 * flutter[0] + 2.0 * lift)
    o.r('right_gills', y=3.0 * flutter[1], z=-(7.0 * flutter[1] + 2.0 * lift))
    o.r('top_gills', x=6.0 * flutter[2])
    o.r('tail', x=-1.0 * lift, y=swing(7.0 * wave(p, 1, -0.05)))
    return o


HAPPY_S = 0.9


def happy(u):
    """0.9 s: crouch, hop with the gills flared and eyes squeezed shut, a
    springy landing, then everything settles back exactly to rest."""
    t = u * HAPPY_S
    o = Pose()
    t0, t1 = 0.13, 0.50  # take-off, touch-down
    crouch = pulse(t, 0.0, 0.20)
    land = pulse(t, t1, t1 + 0.18)
    air = clamp((t - t0) / (t1 - t0))
    o.hop = HOP_MAX * 4 * air * (1 - air) if t0 < t < t1 else 0.0
    o.pitch = 6.0 * math.sin(math.pi * air) * (1 - 1.5 * air) if t0 < t < t1 else 0.0
    # After touch-down the head nods on a spring that has died out by the end.
    nod = 0.0
    if t > t1:
        nod = 2.5 * (1 - window(t, t1 + 0.04, HAPPY_S)) ** 2 * math.sin(TAU * (t - t1) / 0.32)
    o.lift = -0.75 * crouch - 0.55 * land + 0.35 * pulse(t, t0, t0 + 0.2)
    o.r('head', x=-4.0 * crouch + 6.0 * pulse(t, t0, t1 + 0.05) - 3.0 * land + nod)
    flare = 24.0 * pulse(t, t0 - 0.03, t1 + 0.18)
    flutter = 7.0 * math.sin(TAU * (t - t0) / 0.24) * pulse(t, t0, HAPPY_S)
    o.gills(flare=flare + flutter, sweep=7.0 * crouch - 5.0 * pulse(t, t0, t1), top=12.0 * pulse(t, t0, t1 + 0.12) + 0.6 * flutter)
    wag = 11.0 * math.sin(TAU * (t - 0.05) / 0.3) * pulse(t, 0.05, HAPPY_S)
    o.r('tail', x=-3.0 * pulse(t, t0, t1), y=swing(wag))
    for leg, side in (('leg1', 1), ('leg2', -1), ('leg3', 1), ('leg4', -1)):
        o.r(leg, z=side * 6.0 * pulse(t, t0, t1))
    o.lid = 0.85 * pulse(t, t0 - 0.02, t1 + 0.20) ** 0.5
    return o


SAD_S = 0.8


def sad(u):
    """0.8 s: a small breath in, then a slump: head down, gills drooping and
    folded back, tail dropped, eyes half shut. Holds the last frame."""
    t = u * SAD_S
    o = Pose()
    sigh = pulse(t, 0.0, 0.30)
    d = smoother((t - 0.12) / (SAD_S - 0.12))
    o.lift = 0.25 * sigh - 0.45 * d
    o.r('head', x=2.0 * sigh - 13.0 * d, y=-3.0 * d, z=4.0 * d)
    o.gills(flare=3.0 * sigh - 18.0 * d, sweep=58.0 * d, top=-30.0 * d)
    o.r('tail', x=8.0 * d, y=-6.0 * d)
    o.lid = 0.5 * d
    return o


MOODS = {
    # name: (pose fn, frames, fps, loops)
    # 30 fps everywhere: it divides 60 Hz evenly, so frames pace evenly.
    'idle': (idle, 72, 30, True),
    'think': (think, 36, 30, True),
    'happy': (happy, 27, 30, False),
    'sad': (sad, 24, 30, False),
}


def phases(frames, loops):
    if loops:
        return [i / frames for i in range(frames)]
    return [i / (frames - 1) for i in range(frames)]


# --- rendering ----------------------------------------------------------------

class Renderer:
    def __init__(self, toolkit):
        src = os.path.join(toolkit, 'src')
        # Appended, not prepended, so nothing in the toolkit can shadow the
        # standard library; no bytecode, so the toolkit folder stays untouched.
        sys.path.append(src)
        sys.dont_write_bytecode = True
        import rig2  # noqa: E402  (the toolkit's renderer)

        self.rig2 = rig2
        with open(os.path.join(src, 'camera.json'), encoding='utf-8') as f:
            cam = json.load(f)
        with open(os.path.join(src, 'rest_pose.json'), encoding='utf-8') as f:
            self.rest = json.load(f)
        rig2.PERSP = cam.get('persp', 0.15)
        self.yaw, self.pitch = cam['yaw'], cam['pitch']
        self.head_mesh = next(i for i, n in enumerate(rig2.NODES) if n['name'] == 'head' and 'mesh' in n)
        self.head_quads = rig2.MESH_Q[self.head_mesh]
        eye = tuple(rig2.EYE)
        self.eyes = [k for k, (_, col, _) in enumerate(self.head_quads) if tuple(col) == eye]
        self.lid_color = self._color_above(self.eyes[0])
        rest = rig2.scene(self.rest)
        self.ground = min(q[0][:, 1].min() for q in rest)
        self.camera = rig2.camera(self.yaw, self.pitch)
        self.center = None
        self.scale = None

    def _color_above(self, k):
        q, _, n = self.head_quads[k]
        c = q.mean(0) + np.array([0, 1 / 16, 0])
        best = min(
            (np.linalg.norm(qq.mean(0) - c), col)
            for j, (qq, col, nn) in enumerate(self.head_quads)
            if j != k and np.allclose(nn, n)
        )
        return best[1]

    def _set_lid(self, lid):
        """Swap the head's eye texels for skin with a shorter dark eye on top,
        anchored to the lower edge so the lid closes downwards."""
        quads = list(self.head_quads)
        if lid > 1e-3:
            keep = max(0.0, 1 - lid) * 0.82 + 0.18
            for k in self.eyes:
                q, col, n = quads[k]
                quads[k] = (q, self.lid_color, n)
                lo = q[:, 1].min()
                s = q.copy()
                s[:, 1] = lo + (s[:, 1] - lo) * keep
                s += n * (0.02 / 16)  # sit just proud of the skin texel
                quads.append((s, col, n))
        self.rig2.MESH_Q[self.head_mesh] = quads

    def _shadow_outline(self, o):
        """The shadow's ellipse on the ground, projected; it shrinks as the
        animal rises."""
        k = 1 - 0.22 * clamp(o.hop / HOP_MAX)
        a = np.linspace(0, TAU, 72, endpoint=False)
        pts = np.stack([SHADOW['rx'] * k * np.cos(a), np.full_like(a, self.ground),
                        SHADOW['cz'] + SHADOW['rz'] * k * np.sin(a)], 1)
        return self.rig2._proj(pts @ self.camera.T)[:, :2]

    def _shadow(self, o, px):
        outline = [((x - self.center[0]) * self.scale * px + px / 2, px / 2 - (y - self.center[1]) * self.scale * px)
                   for x, y in self._shadow_outline(o)]
        mask = Image.new('L', (px, px), 0)
        ImageDraw.Draw(mask).polygon(outline, fill=255)
        mask = mask.filter(ImageFilter.GaussianBlur(px * SHADOW['blur']))
        strength = SHADOW['alpha'] * (1 - 0.55 * clamp(o.hop / HOP_MAX))
        layer = Image.new('RGBA', (px, px), SHADOW['rgb'] + (0,))
        layer.putalpha(mask.point(lambda v: round(v * strength)))
        return layer

    def bounds(self, o):
        pts = self.rig2._proj(np.concatenate([q[0] for q in self.rig2.scene(o.build(self.rest))]) @ self.camera.T)[:, :2]
        pts = np.concatenate([pts, self._shadow_outline(o)])
        return pts.min(0), pts.max(0)

    def frame(self, o, size):
        pose = o.build(self.rest)
        px = size * SUPERSAMPLE
        self._set_lid(o.lid)
        try:
            body = self.rig2.render(pose, yaw=self.yaw, pitch=self.pitch, size=px, ss=1,
                                    center=self.center, scale=self.scale)
        finally:
            self.rig2.MESH_Q[self.head_mesh] = self.head_quads
        img = self._shadow(o, px)
        img.alpha_composite(body)
        # Pillow premultiplies alpha when resampling RGBA, so edges stay clean.
        return img.resize((size, size), Image.Resampling.BOX)

    def fit(self, poses):
        lo = np.array([np.inf, np.inf])
        hi = -lo
        for o in poses:
            a, b = self.bounds(o)
            lo = np.minimum(lo, a)
            hi = np.maximum(hi, b)
        self.center = (lo + hi) / 2
        self.scale = (1 - 2 * MARGIN) / (hi - lo).max()
        return lo, hi


def edge_alpha(img):
    a = np.asarray(img)[..., 3]
    return int(max(a[0].max(), a[-1].max(), a[:, 0].max(), a[:, -1].max()))


def strip(frames):
    w, h = frames[0].size
    out = Image.new('RGBA', (w * len(frames), h), (0, 0, 0, 0))
    for i, f in enumerate(frames):
        out.paste(f, (i * w, 0))
    return out


def quantize(img, colors=256, shadow_levels=40, iterations=24, seed=7):
    """Palette PNG with alpha, one palette per strip so frames never flicker.

    Pillow's octree bands the soft shadow, so the palette is built here: an
    even ramp of alpha steps in the shadow's colour, and weighted k-means in
    premultiplied RGBA for everything else (the voxels' flat colours and their
    anti-aliased edges)."""
    a = np.asarray(img).reshape(-1, 4)
    uniq, inverse, counts = np.unique(a, axis=0, return_inverse=True, return_counts=True)
    inverse = inverse.reshape(-1)
    rgb = np.array(SHADOW['rgb'])
    top = round(255 * SHADOW['alpha'])
    clear = uniq[:, 3] == 0
    shade = ~clear & (np.abs(uniq[:, :3].astype(int) - rgb).max(1) <= 2) & (uniq[:, 3] <= top + 1)
    rest = ~clear & ~shade

    ramp = np.linspace(1, top, shadow_levels)
    pal = [np.array([0, 0, 0, 0], float)] + [np.array([*rgb, v]) for v in ramp]
    index = np.zeros(len(uniq), int)
    index[shade] = 1 + np.abs(uniq[shade, 3][:, None] - ramp[None]).argmin(1)

    k = colors - len(pal)
    pts = uniq[rest].astype(float)
    w = counts[rest].astype(float)
    pm = np.concatenate([pts[:, :3] * pts[:, 3:] / 255, pts[:, 3:]], 1)
    if len(pm) <= k:
        cent = pm
        label = np.arange(len(pm))
    else:
        rng = np.random.default_rng(seed)
        # k-means++ seeding, weighted by pixel count.
        cent = [pm[np.argmax(w)]]
        d = ((pm - cent[0]) ** 2).sum(1)
        for _ in range(k - 1):
            pr = d * w
            cent.append(pm[rng.choice(len(pm), p=pr / pr.sum())])
            d = np.minimum(d, ((pm - cent[-1]) ** 2).sum(1))
        cent = np.array(cent)

        def nearest(c):
            return ((pm * pm).sum(1)[:, None] - 2 * pm @ c.T + (c * c).sum(1)[None]).argmin(1)

        for _ in range(iterations):
            label = nearest(cent)
            mass = np.bincount(label, weights=w, minlength=k)
            used = mass > 0
            for ch in range(4):
                sums = np.bincount(label, weights=w * pm[:, ch], minlength=k)
                cent[used, ch] = sums[used] / mass[used]
        label = nearest(cent)
    index[rest] = len(pal) + label
    for c in cent:
        alpha = max(c[3], 1e-6)
        pal.append(np.array([*(c[:3] * 255 / alpha), c[3]]))
    pal = np.clip(np.round(np.array(pal)), 0, 255).astype(np.uint8)

    out = Image.fromarray(index[inverse].astype(np.uint8).reshape(img.height, img.width), 'P')
    out.putpalette(pal[:, :3].reshape(-1).tolist())
    return out, bytes(pal[:, 3].tolist())


def save_png(img, path):
    q, alpha = quantize(img)
    q.save(path, optimize=True, transparency=alpha)
    return os.path.getsize(path)


def preview(name, frames, fps, loops, out):
    os.makedirs(out, exist_ok=True)
    for bg, tag in (((16, 18, 21), 'dark'), ((235, 235, 235), 'light')):
        cols = 12
        w, h = frames[0].size
        rows = (len(frames) + cols - 1) // cols
        sheet = Image.new('RGBA', (cols * w, rows * h), bg + (255,))
        for i, f in enumerate(frames):
            sheet.alpha_composite(f, ((i % cols) * w, (i // cols) * h))
        sheet.save(os.path.join(out, f'{name}-sheet-{tag}.png'))
    flat = []
    for f in frames:
        b = Image.new('RGBA', f.size, (16, 18, 21, 255))
        b.alpha_composite(f)
        flat.append(b.convert('RGB'))
    flat[0].save(os.path.join(out, f'{name}.gif'), save_all=True, append_images=flat[1:],
                 duration=round(1000 / fps), loop=0 if loops else 1, disposal=1)


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('toolkit', help='path to the 美西螈表情包工具包 folder')
    ap.add_argument('--out', default=os.path.join(here, '..', 'src', 'mascot'))
    ap.add_argument('--preview', help='also write contact sheets and GIFs here')
    args = ap.parse_args()

    r = Renderer(os.path.abspath(args.toolkit))
    sampled = {name: [fn(p) for p in phases(n, loops)] for name, (fn, n, fps, loops) in MOODS.items()}
    lo, hi = r.fit([o for poses in sampled.values() for o in poses])
    print(f'framing: centre {r.center.round(4).tolist()}, scale {r.scale:.4f}, union {(hi - lo).round(3).tolist()}')

    os.makedirs(args.out, exist_ok=True)
    total = 0
    for name, (fn, n, fps, loops) in MOODS.items():
        for size, suffix in ((HERO, ''), (SMALL, '-sm')):
            frames = [r.frame(o, size) for o in sampled[name]]
            edge = max(edge_alpha(f) for f in frames)
            if edge > 2:
                print(f'warning: {name}{suffix} touches the frame edge (alpha {edge})')
            path = os.path.join(args.out, f'{name}{suffix}.png')
            nbytes = save_png(strip(frames), path)
            total += nbytes
            print(f'{name}{suffix}.png: {n} frames of {size}px, {n / fps:.2f} s at {fps} fps, {nbytes / 1024:.1f} KB')
            if args.preview:
                preview(name + suffix, frames, fps, loops, args.preview)
    print(f'total {total / 1024:.1f} KB')


if __name__ == '__main__':
    main()
