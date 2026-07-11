<?php

/**
 * IDE Helper for Imagick extension.
 * This file is not loaded at runtime if the extension is present,
 * it is only used to provide autocomplete and silence IDE warnings
 * when the ext-imagick php extension is not installed locally.
 */

if (!class_exists('Imagick', false)) {
    class Imagick
    {
        const COMPOSITE_OVER = 40;
        const CHANNEL_DEFAULT = 134217727;

        public function setResolution($x_resolution, $y_resolution) {}
        public function readImage($filename) {}
        public function setImageFormat($format) {}
        public function thumbnailImage($columns, $rows, $bestfit = false, $fill = false, $legacy = false) {}
        public function getImageWidth() {}
        public function getImageHeight() {}
        public function newImage($cols, $rows, $background_color, $format = null) {}
        public function compositeImage($composite_object, $composite, $x, $y, $channel = self::CHANNEL_DEFAULT) {}
        public function writeImage($filename = null) {}
        public function clear() {}
        public function destroy() {}
    }
}

if (!class_exists('ImagickPixel', false)) {
    class ImagickPixel
    {
        public function __construct($color = null) {}
    }
}
