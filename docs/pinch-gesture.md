# Mathematics Behind Pinch Gesture

## Problem Statement (2D)

**Given**:

- $\mathbf{p}_1, \mathbf{p}_2 \in \mathbb{R}^2$ — 2D screen coordinates for the gesture start
- $\mathbf{q}_1, \mathbf{q}_2 \in \mathbb{R}^2$ — 2D screen coordinates for the gesture end
- $M \in \mathbb{R}^{4 \times 4}$ — known transform (uniform scale, rotation about $Z$, translation)
- $P \in \mathbb{R}^{2 \times 4}$ — **orthographic projection** that keeps only $x,y$ coordinates:
```math
  P =
  \begin{bmatrix}
    1 & 0 & 0 & 0 \\
    0 & 1 & 0 & 0 \\
  \end{bmatrix} = 
  \begin{bmatrix}
    I_{2\times 2} & \mathbf{0}_{2\times 1} & \mathbf{0}_{2\times 1}
  \end{bmatrix}
```

- $\mathbf{w}_1, \mathbf{w}_2 \in \mathbb{R}^4$ — 3D points in **homogeneous coordinates**.
- $D \in \mathbb{R}^{4 \times 4}$ — **unknown transform** to be found ($s$: uniform scale, $\phi$: rotation angle about Z, $\mathbf{t}$: translation ) so that:
```math
  \begin{align*}
  \mathbf{p}_i &=  P \, M \, \mathbf{w}_i \\
  \mathbf{q}_i &= P \, M\, D\, \mathbf{w}_i
  \end{align*}
```

**Goal**: Find the properties $s, \phi, \mathbf{t}$ of the $D$ affine transformation

### Solution

$\mathbf{w}_1$ can be found as the inverse projection of $\mathbf{p}_1$. The inverse projection defines a ray passing through the $\mathbf{p}_i$ screen point, but for orthographic projection we can use the
intersection of the ray with the near (or far) plane and the z component will be eliminated as we will see.

Define:

```math
\Delta := M^{-1} \, D \, M
```

Then:

```math
\mathbf{q}_i = P \, M \, D \, \mathbf{w}_i
= P \, \Delta \, M \, \mathbf{w}_i \\
\mathbf{v}_i = \begin{bmatrix} v_x \\ v_y \\ v_z \\ 1 \end{bmatrix} =  M \, \mathbf{w}_i
```

The problem reduces to:

```math
\begin{align*}
\mathbf{p}_i &= \begin{bmatrix} p_x \\ p_y \end{bmatrix} =  P \, \mathbf{v}_i \\
\mathbf{q}_i &= \begin{bmatrix} q_x \\ q_y \end{bmatrix} =  P \, \Delta \, \mathbf{v}_i
\end{align*}
```

Once $\Delta$ is found:

```math
D = M \, \Delta \, M^{-1}
```

**Assumptions (as given):**
- $M$ is uniform scale, rotation about Z, translation (so it does **not** mix $z$ into $x,y$).
- $D$ (and thus $F$) is uniform scale, rotation about Z, translation.
- $P$ is the orthographic projector that selects $x,y$.

```math
\begin{align*}
\Delta &=
\begin{bmatrix}
  s\cos\theta & -s\sin\theta & 0 & t_x \\
  s\sin\theta & \;\;s\cos\theta & 0 & t_y \\
  0 & 0 & s & t_z \\
  0 & 0 & 0 & 1
\end{bmatrix} =
\begin{bmatrix}
  sR_{2\times 2}(\phi) & 0_{2\times 1} & \mathbf{t}_{xy} \\
  0_{1\times 2} & s & t_z \\
  0_{1\times 2} & 0 & 1
\end{bmatrix} \\

P \, \Delta &= 
\begin{bmatrix}
  sR_{2\times 2}(\phi) & 0_{2\times 1} & \mathbf{t}_{xy} \\
  0_{1\times 2} & s & t_z \\
  0_{1\times 2} & 0 & 1
\end{bmatrix}
\begin{bmatrix}
    I_{2\times 2} & \mathbf{0}_{2\times 1} & \mathbf{0}_{2\times 1}
\end{bmatrix}
 = 
\begin{bmatrix}
  sR_{2\times 2}(\phi) & \mathbf{0}_{2\times 1} & \mathbf{t}_{xy}
\end{bmatrix} \\

 P \, \Delta \, \mathbf{v}_i &= 
\begin{bmatrix}
  sR_{2\times 2}(\phi) & \mathbf{0}_{2\times 1} & \mathbf{t}_{xy}
\end{bmatrix} 
\begin{bmatrix}
  v_x \\ v_y \\ v_z \\ 1
\end{bmatrix} =
sR_{2\times 2}(\phi) \, \begin{bmatrix} v_x \\ v_y \end{bmatrix} + \mathbf{t}_{xy}

\end{align*} 
```

Since $P$ selects the $x,y$ component 

```math
\begin{bmatrix} p_x \\ p_y \end{bmatrix} = P \, \mathbf{v}_i = \begin{bmatrix} v_x \\ v_y \end{bmatrix}
```

Then we can write

```math
\mathbf{q}_i = sR_{2\times 2}(\phi) \, \mathbf{p}_i + \mathbf{t}_{xy} \quad  i \in \{1,2\},
```

By subtraction the two equation we can eliminate the translation and find $s, qphi$. 

```math
\begin{align*} 
\mathbf{q}_{2} - \mathbf{q}_{1} = sR(\phi)(\mathbf{p}_{2} - \mathbf{p}_{1})  \\
\end{align*} 
```

The rotation does not change the length of vectors, and scale dose not change the angle, thus the parameters of the $\Delta$ transformation:

```math
\boxed{
\begin{align*}
s &= \frac{\lVert \mathbf{q}_{2} - \mathbf{q}_{1} \rVert}{\lVert \mathbf{p}_{2} - \mathbf{p}_{1} \rVert} \\
\phi &= \text{atan2}(\mathbf{q}_{2} - \mathbf{q}_{1}) - \text{atan2}(\mathbf{p}_{2} - \mathbf{p}_{1}) \\
\mathbf{t}_{xy} &= \mathbf{q}_{1} - sR_2(\phi) \, \mathbf{p}_{1}  \\
\end{align*}
}
```

## Bevy

In Bevy the camera transform ($C$) is the $M^{-1}$, thus to apply $D, \Delta$ we have

```math
\begin{align*}
C_1 &= M^{-1} \\
C_2 &= (D \, M)^{-1} = M^{-1} \, D^{-1} = C_1 \, D^{-1} \\
C_2 &= (M \, \Delta)^{-1} = \Delta^{-1} \, M^{-1} = \Delta^{-1} \, C_1
\end{align*}
```

The inverse of a transformation:

```math
\begin{align*}
x' &= sR(\phi)x + \mathbf{t}_{xy} \\
x' - \mathbf{t}_{xy} &= sR(\phi)x \\
(sR(\phi))^{-1}(x' - \mathbf{t}_{xy}) &= x \\
\frac{1}{s}R(-\phi)(x' - \mathbf{t}_{xy}) &= x \\
\frac{1}{s}R(-\phi)x' - \frac{1}{s}R(-\phi) \, \mathbf{t}_{xy} &= x \\
\end{align*}
```

Finally the parameters of the inverse transformation:

```math
\boxed{
\begin{align*}
s' &= \frac{1}{s} \\
\phi' &= -\phi \\
\mathbf{t}'_{xy} &= -\frac{1}{s}R(-\phi) \, \mathbf{t}_{xy}
\end{align*}
}
```

The gesture and other positions are usually given in screen coordinates a.k.a viewport (origin is at the top left corner, y points downward and measured in pixels), but the above calculation is valid for the screen-centered coordinates (origin is at the center of the screen, y points upward and measured in pixel).
