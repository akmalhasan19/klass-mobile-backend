<?php

namespace Tests\Concerns;

use Illuminate\Support\Str;
use RuntimeException;
use ZipArchive;

trait CreatesMediaGenerationArtifacts
{
    private function createTempArtifactFile(string $extension): string
    {
        return match (strtolower($extension)) {
            'pdf' => $this->createTempPdfArtifact(),
            'docx' => $this->createTempOfficeArtifact('docx', [
                '[Content_Types].xml' => <<<'XML'
<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
    <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
    <Default Extension="xml" ContentType="application/xml"/>
    <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>
XML,
                '_rels/.rels' => <<<'XML'
<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
    <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>
XML,
                'word/document.xml' => <<<'XML'
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
    <w:body>
        <w:p>
            <w:r>
                <w:t>Klass media generation artifact</w:t>
            </w:r>
        </w:p>
    </w:body>
</w:document>
XML,
            ]),
            'pptx' => $this->createTempOfficeArtifact('pptx', [
                '[Content_Types].xml' => <<<'XML'
<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
    <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
    <Default Extension="xml" ContentType="application/xml"/>
    <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
    <Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
</Types>
XML,
                '_rels/.rels' => <<<'XML'
<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
    <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>
XML,
                'ppt/presentation.xml' => <<<'XML'
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
    <p:sldIdLst>
        <p:sldId id="256" r:id="rId1"/>
    </p:sldIdLst>
</p:presentation>
XML,
                'ppt/slides/slide1.xml' => <<<'XML'
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
    <p:cSld>
        <p:spTree/>
    </p:cSld>
</p:sld>
XML,
            ]),
            default => throw new RuntimeException('Unsupported artifact extension for tests: ' . $extension),
        };
    }

    private function createTempPdfArtifact(): string
    {
        $path = sys_get_temp_dir() . '/media_generation_' . Str::random(12) . '.pdf';

        file_put_contents($path, implode("\n", [
            '%PDF-1.4',
            '1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj',
            '2 0 obj << /Type /Pages /Count 1 /Kids [3 0 R] >> endobj',
            '3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] >> endobj',
            'xref',
            '0 4',
            '0000000000 65535 f ',
            'trailer << /Root 1 0 R /Size 4 >>',
            'startxref',
            '128',
            '%%EOF',
        ]));

        return $path;
    }

    /**
     * @param  array<string, string>  $entries
     */
    private function createTempOfficeArtifact(string $extension, array $entries): string
    {
        $path = sys_get_temp_dir() . '/media_generation_' . Str::random(12) . '.' . $extension;

        if (! class_exists(ZipArchive::class)) {
            file_put_contents($path, "PK\x03\x04klass-media-generation-office-artifact");

            return $path;
        }

        $zip = new ZipArchive();

        if ($zip->open($path, ZipArchive::CREATE | ZipArchive::OVERWRITE) !== true) {
            throw new RuntimeException('Could not create temporary Office artifact for tests.');
        }

        foreach ($entries as $entryPath => $contents) {
            $zip->addFromString($entryPath, $contents);
        }

        $zip->close();

        return $path;
    }
}