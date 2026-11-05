"""Renders the Agent Window mascot: the official Axolotl Launcher axolotl,
posed with the launcher's sticker toolkit and saved as horizontal sprite
strips that src/mascot.css plays with steps().

    python packages/agent-window/dev/render-mascot.py <toolkit> [--out DIR] [--preview DIR]

<toolkit> is the 美西螈表情包工具包 folder (it holds src/rig2.py,
src/rest_pose.json, src/camera.json and model/axolotl.gltf). Only numpy and
Pillow are needed. Strips go to src/mascot/ next to this script unless --out
says otherwise; --preview also writes contact sheets and GIFs for review.

Every frame of every mood shares one camera, scale and centre, measured from
the union of all poses, so nothing jumps between frames or moods and the hop,
the wave and gill flares never clip. Each mood is rendered only at the sizes
listed for it in MOODS; a strip left over from an earlier table is reported
so it can be deleted along with its ASSETS entry in src/lib.rs. If you change
a motion, a frame count or a size here, mirror it in src/mascot.css and in
PLAYED in src/mascot.js.
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


def back(x, k=1.6):
    """0 -> 1 with a small overshoot that settles exactly on 1."""
    x = clamp(x) - 1
    return 1 + (k + 1) * x ** 3 + k * x ** 2


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
    `hop` raises the whole animal; `pitch` and `roll` tip it about the ground
    under its middle; `lid` closes the eyes (0 open, 1 shut)."""

    def __init__(self):
        self.rot = {}
        self.lift = 0.0
        self.hop = 0.0
        self.pitch = 0.0
        self.roll = 0.0
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
        if self.hop or self.pitch or self.roll:
            pose['root'] = {'rot': [self.pitch, 0, self.roll], 'move': [0, self.hop, 0]}
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


HELLO_S = 1.5


def hello(u):
    """1.5 s: a small springy hop (under half of happy's) with the gills
    flared and the eyes squeezed shut, a squashy landing, then a chin lift
    with a friendly head tilt while the near front leg rises beside the cheek
    and waves twice. It lowers and settles exactly back to rest."""
    t = u * HELLO_S
    o = Pose()
    t0, t1 = 0.12, 0.38  # take-off, touch-down
    crouch = pulse(t, 0.0, 0.18)
    land = pulse(t, t1, t1 + 0.18)
    air = clamp((t - t0) / (t1 - t0))
    hop = math.sin(math.pi * air) if t0 < t < t1 else 0.0
    o.hop = 0.45 * HOP_MAX * 4 * air * (1 - air) if t0 < t < t1 else 0.0
    o.pitch = 4.0 * hop * (1 - 1.5 * air)
    face = window(t, 0.30, 0.52) * (1 - window(t, 1.12, 1.42))
    up = window(t, 0.36, 0.58) * (1 - window(t, 1.10, 1.32))
    env = window(t, 0.48, 0.60) * (1 - window(t, 1.00, 1.12))
    w = math.sin(TAU * (t - 0.56) / 0.26) * env
    lag = math.sin(TAU * (t - 0.61) / 0.26) * env
    o.lift = -0.7 * crouch - 0.55 * land + 0.3 * pulse(t, t0, t0 + 0.18) + 0.22 * face
    o.r('head', x=-4.0 * crouch + 5.0 * hop - 3.0 * land + 3.0 * face - 1.5 * pulse(t, 1.28, HELLO_S),
        y=2.0 * face, z=-4.0 * face + 1.0 * lag)
    # The near front leg sits under the cheek, so the paw waves between low
    # and out (against the background) and high against the side of the
    # head, where the perked gills have made room for it.
    o.r('leg4', x=(50.0 + 24.0 * w) * up, y=(-35.0 + 6.0 * w) * up, z=(-5.0 - 6.0 * w) * up)
    for leg, side in (('leg1', 1), ('leg2', -1), ('leg3', 1), ('leg4', -1)):
        o.r(leg, z=side * 5.0 * pulse(t, t0, t1))
    pop = pulse(t, t0 - 0.03, t1 + 0.1)
    o.gills(flare=max(24.0 * pop, 20.0 * face) + 4.0 * lag, sweep=6.0 * crouch - 4.0 * hop - 3.0 * face,
            top=max(12.0 * pop, 10.0 * face) + 3.0 * lag)
    wag = 9.0 * math.sin(TAU * (t - 0.05) / 0.32) * pulse(t, 0.05, 1.45)
    o.r('tail', x=-3.0 * hop - 2.0 * face, y=swing(wag))
    o.lid = 0.85 * pulse(t, t0 - 0.02, t1 + 0.12) ** 0.5
    return o


LOOK_S = 1.8


def gaze(t):
    """Head yaw for look: one side, a beat, the other side, back to rest. The
    rest pose already faces the viewer three-quarters, so the first glance
    turns past the viewer to the right and the second to the left."""
    return 26.0 * back((t - 0.04) / 0.32) - 42.0 * back((t - 0.60) / 0.42) + 16.0 * back((t - 1.22) / 0.38)


def look(u):
    """1.8 s: glances to one side, holds a beat, glances to the other with a
    blink on the way, holds, then turns back to rest. The gills trail each
    turn and the tail answers it."""
    t = u * LOOK_S
    o = Pose()
    g = gaze(t)
    trail = gaze(t - 0.07) - g
    away = min(1.0, abs(g) / 16.0)
    o.lift = 0.15 * away
    o.r('head', x=2.0 * away, y=g, z=-0.15 * g)
    o.r('left_gills', y=0.9 * trail, z=0.3 * trail)
    o.r('right_gills', y=0.9 * trail, z=-0.3 * trail)
    o.r('top_gills', x=0.3 * abs(trail), y=0.6 * trail)
    o.r('tail', y=swing(-0.3 * gaze(t - 0.12)))
    o.lid = blink(t, 0.64, 0.16)
    return o


ATTEND_S = 0.5


def attend(u):
    """0.5 s, then held: perks up to listen. It leans in, the head lifts and
    turns a little towards the viewer, the gills tip forward a beat later and
    the eyes stay wide open. The last frame is the held pose."""
    t = u * ATTEND_S
    o = Pose()
    e = back(t / 0.36, 1.4)
    g = back((t - 0.04) / 0.38, 2.0)
    o.pitch = -3.0 * e
    o.lift = 0.3 * e - 0.08 * pulse(t, 0.0, 0.1)
    o.r('head', x=5.0 * e, y=7.0 * e, z=-2.5 * e)
    o.gills(flare=9.0 * g, sweep=-8.0 * g, top=12.0 * g)
    o.r('tail', x=-2.0 * e, y=swing(3.0 * g))
    return o


def work(p):
    """0.9 s: busy, focused paddling. The legs stroke in diagonal pairs, the
    body rocks with each stroke, the tail sweeps and the gills stream back
    while the head stays low and steady."""
    o = Pose()
    o.roll = 2.5 * wave(p, 2, -0.08)
    o.lift = 0.12 * (0.5 - 0.5 * math.cos(TAU * 4 * p))
    o.r('head', x=-2.5 + 1.0 * wave(p, 4, -0.1), y=-1.5 * wave(p, 2, -0.05))
    # Front and back legs on the same side mirror their y axis, as in the
    # toolkit's swimming poses; leg3 and leg2 stroke together, then leg4 and leg1.
    for leg, side, phase in (('leg3', 1, 0.0), ('leg4', -1, 0.5), ('leg1', -1, 0.5), ('leg2', 1, 0.0)):
        o.r(leg, y=side * 30.0 * wave(p, 2, phase))
        if leg in ('leg3', 'leg4'):  # front paws lift on the way forward
            o.r(leg, x=10.0 * max(0.0, wave(p, 2, phase + 0.25)))
    o.gills(flare=4.0 * wave(p, 4, -0.1), sweep=6.0 + 3.0 * wave(p, 4, -0.15), top=-6.0 + 3.0 * wave(p, 4, -0.2))
    o.r('tail', x=-1.0, y=swing(10.0 * wave(p, 2, -0.12)))
    return o


NOD_S = 0.7


def bob(t):
    """Head pitch for nod: two dips, the second smaller, and a small rebound."""
    return -14.0 * pulse(t, 0.0, 0.25) - 10.0 * pulse(t, 0.22, 0.46) + 2.5 * pulse(t, 0.44, 0.62)


def nod(u):
    """0.7 s: two small yes-nods with the gills bobbing a beat behind and a
    content squint, settling exactly on rest."""
    t = u * NOD_S
    o = Pose()
    n = bob(t)
    trail = bob(t - 0.06) - n
    o.lift = 0.02 * n
    o.r('head', x=n)
    o.gills(flare=0.8 * trail, top=1.2 * trail)
    o.r('tail', x=-0.3 * trail)
    o.lid = 0.4 * pulse(t, 0.02, 0.56)
    return o


SIZES = {'hero': (HERO, ''), 'small': (SMALL, '-sm')}
MOODS = {
    # name: (pose fn, frames, fps, loops, sizes)
    # 30 fps everywhere: it divides 60 Hz evenly, so frames pace evenly.
    # Each mood is rendered only at the sizes that play it: the 96 px hero
    # greets, looks around and listens on the new-task page; the 28 px mark
    # thinks, works and reacts in the status line.
    'idle': (idle, 72, 30, True, ('hero', 'small')),
    'think': (think, 36, 30, True, ('small',)),
    'happy': (happy, 27, 30, False, ('hero', 'small')),
    'sad': (sad, 24, 30, False, ('small',)),
    'hello': (hello, 45, 30, False, ('hero',)),
    'look': (look, 54, 30, False, ('hero',)),
    'attend': (attend, 15, 30, False, ('hero',)),
    'work': (work, 27, 30, True, ('small',)),
    'nod': (nod, 21, 30, False, ('small',)),
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
    sampled = {name: [fn(p) for p in phases(n, loops)] for name, (fn, n, fps, loops, _) in MOODS.items()}
    lo, hi = r.fit([o for poses in sampled.values() for o in poses])
    print(f'framing: centre {r.center.round(4).tolist()}, scale {r.scale:.4f}, union {(hi - lo).round(3).tolist()}')

    os.makedirs(args.out, exist_ok=True)
    total = 0
    written = set()
    for name, (fn, n, fps, loops, sizes) in MOODS.items():
        for size, suffix in (SIZES[s] for s in sizes):
            frames = [r.frame(o, size) for o in sampled[name]]
            edge = max(edge_alpha(f) for f in frames)
            if edge > 2:
                print(f'warning: {name}{suffix} touches the frame edge (alpha {edge})')
            path = os.path.join(args.out, f'{name}{suffix}.png')
            nbytes = save_png(strip(frames), path)
            total += nbytes
            written.add(f'{name}{suffix}.png')
            print(f'{name}{suffix}.png: {n} frames of {size}px, {n / fps:.2f} s at {fps} fps, {nbytes / 1024:.1f} KB')
            if args.preview:
                preview(name + suffix, frames, fps, loops, args.preview)
    print(f'total {total / 1024:.1f} KB')
    for name in sorted(set(os.listdir(args.out)) - written):
        if name.endswith('.png'):
            print(f'warning: {name} is no longer rendered; delete it and its ASSETS entry in src/lib.rs')


if __name__ == '__main__':
    main()
