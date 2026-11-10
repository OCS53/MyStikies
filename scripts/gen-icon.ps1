Add-Type -AssemblyName System.Drawing

function New-RoundedPath([single]$x, [single]$y, [single]$w, [single]$h, [single]$r) {
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = $r * 2
    $path.AddArc($x, $y, $d, $d, 180, 90)
    $path.AddArc($x + $w - $d, $y, $d, $d, 270, 90)
    $path.AddArc($x + $w - $d, $y + $h - $d, $d, $d, 0, 90)
    $path.AddArc($x, $y + $h - $d, $d, $d, 90, 90)
    $path.CloseFigure()
    return $path
}

function New-StickyBitmap([int]$size) {
    $bmp = New-Object System.Drawing.Bitmap $size, $size
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.Clear([System.Drawing.Color]::Transparent)
    $pad = [single]($size * 0.08)
    $side = [single]($size - (2 * $pad))
    $radius = [single]($size * 0.18)
    $path = New-RoundedPath $pad $pad $side $side $radius
    $rect = New-Object System.Drawing.RectangleF $pad, $pad, $side, $side
    $brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
        $rect,
        [System.Drawing.Color]::FromArgb(255, 255, 228, 150),
        [System.Drawing.Color]::FromArgb(255, 245, 196, 78),
        45
    )
    $g.FillPath($brush, $path)
    $fold = [single]($size * 0.28)
    $foldPath = New-Object System.Drawing.Drawing2D.GraphicsPath
    $right = $pad + $side
    $foldPath.AddPolygon(@(
        (New-Object System.Drawing.PointF ($right - $fold), $pad),
        (New-Object System.Drawing.PointF $right, $pad),
        (New-Object System.Drawing.PointF $right, ($pad + $fold))
    ))
    $foldBrush = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(90, 120, 78, 20))
    $g.SetClip($path)
    $g.FillPath($foldBrush, $foldPath)
    $g.ResetClip()
    if ($size -ge 48) {
        $line = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(120, 90, 58, 16), [single]([Math]::Max(1, $size * 0.035)))
        $line.StartCap = "Round"
        $line.EndCap = "Round"
        $left = $pad + ($side * 0.18)
        $lineRight = $pad + ($side * 0.72)
        0..2 | ForEach-Object {
            $y = $pad + ($side * (0.38 + $_ * 0.16))
            $g.DrawLine($line, $left, $y, $lineRight, $y)
        }
        $line.Dispose()
    }
    $g.Dispose()
    $brush.Dispose()
    $foldBrush.Dispose()
    $path.Dispose()
    $foldPath.Dispose()
    return $bmp
}

$root = Split-Path -Parent $PSScriptRoot
$iconDir = Join-Path $root "src-tauri\icons"
New-Item -ItemType Directory -Force -Path $iconDir | Out-Null

$pngs = @{
    16 = "16.png"
    32 = "32x32.png"
    48 = "48.png"
    128 = "128x128.png"
    256 = "128x128@2x.png"
}
$files = @()
foreach ($size in ($pngs.Keys | Sort-Object)) {
    $bmp = New-StickyBitmap $size
    $path = Join-Path $iconDir $pngs[$size]
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    if ($size -in 16, 32, 48, 256) { $files += $path }
}

function Get-IconDib($bmp) {
    $w = $bmp.Width
    $h = $bmp.Height
    $xorSize = $w * $h * 4
    $maskStride = [int]([Math]::Ceiling($w / 32.0) * 4)
    $maskSize = $maskStride * $h
    $ms = New-Object System.IO.MemoryStream
    $bw = New-Object System.IO.BinaryWriter $ms
    $bw.Write([UInt32]40)
    $bw.Write([Int32]$w)
    $bw.Write([Int32]($h * 2))
    $bw.Write([UInt16]1)
    $bw.Write([UInt16]32)
    $bw.Write([UInt32]0)
    $bw.Write([UInt32]$xorSize)
    $bw.Write([Int32]0)
    $bw.Write([Int32]0)
    $bw.Write([UInt32]0)
    $bw.Write([UInt32]0)
    for ($y = $h - 1; $y -ge 0; $y--) {
        for ($x = 0; $x -lt $w; $x++) {
            $c = $bmp.GetPixel($x, $y)
            $bw.Write([byte]$c.B)
            $bw.Write([byte]$c.G)
            $bw.Write([byte]$c.R)
            $bw.Write([byte]$c.A)
        }
    }
    $bw.Write((New-Object byte[] $maskSize))
    $bw.Flush()
    return ,$ms.ToArray()
}

$dibSizes = @(16, 32, 48, 256)
$dibs = foreach ($size in $dibSizes) {
    $bmp = New-StickyBitmap $size
    $dib = Get-IconDib $bmp
    $bmp.Dispose()
    ,$dib
}
$ms = New-Object System.IO.MemoryStream
$bw = New-Object System.IO.BinaryWriter $ms
$bw.Write([UInt16]0)
$bw.Write([UInt16]1)
$bw.Write([UInt16]$dibs.Count)
$offset = 6 + (16 * $dibs.Count)
for ($i = 0; $i -lt $dibs.Count; $i++) {
    $side = [byte]($(if ($dibSizes[$i] -ge 256) { 0 } else { $dibSizes[$i] }))
    $bw.Write($side)
    $bw.Write($side)
    $bw.Write([byte]0)
    $bw.Write([byte]0)
    $bw.Write([UInt16]1)
    $bw.Write([UInt16]32)
    $bw.Write([UInt32]$dibs[$i].Length)
    $bw.Write([UInt32]$offset)
    $offset += $dibs[$i].Length
}
foreach ($dib in $dibs) { $bw.Write($dib) }
$bw.Flush()
[System.IO.File]::WriteAllBytes((Join-Path $iconDir "icon.ico"), $ms.ToArray())

function Write-Be32($bw, [int]$value) {
    $bw.Write([byte](($value -shr 24) -band 255))
    $bw.Write([byte](($value -shr 16) -band 255))
    $bw.Write([byte](($value -shr 8) -band 255))
    $bw.Write([byte]($value -band 255))
}

$icnsTypes = [ordered]@{
    icp4 = 16
    icp5 = 32
    ic11 = 32
    ic12 = 64
    ic07 = 128
    ic08 = 256
    ic13 = 256
    ic09 = 512
    ic14 = 512
    ic10 = 1024
}
$entries = New-Object System.Collections.Generic.List[byte[]]
foreach ($type in $icnsTypes.Keys) {
    $bmp = New-StickyBitmap $icnsTypes[$type]
    $png = New-Object System.IO.MemoryStream
    $bmp.Save($png, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    $body = $png.ToArray()
    $png.Dispose()
    $entry = New-Object System.IO.MemoryStream
    $ew = New-Object System.IO.BinaryWriter $entry
    $ew.Write([System.Text.Encoding]::ASCII.GetBytes($type))
    Write-Be32 $ew ($body.Length + 8)
    $ew.Write($body)
    $ew.Flush()
    $entries.Add($entry.ToArray())
}
$payload = ($entries | ForEach-Object { $_.Length } | Measure-Object -Sum).Sum
$icns = New-Object System.IO.MemoryStream
$iw = New-Object System.IO.BinaryWriter $icns
$iw.Write([System.Text.Encoding]::ASCII.GetBytes("icns"))
Write-Be32 $iw ($payload + 8)
foreach ($entry in $entries) { $iw.Write($entry) }
$iw.Flush()
[System.IO.File]::WriteAllBytes((Join-Path $iconDir "icon.icns"), $icns.ToArray())
Write-Output "icons ready"
