<?php

namespace Tests\Unit\MediaGeneration;

use App\MediaGeneration\MediaGeneratedContentGuard;
use Tests\TestCase;

class MediaGeneratedContentGuardTest extends TestCase
{
    public function test_it_rejects_procedural_meta_instructions()
    {
        $testCases = [
            'Follow these steps to teach the lesson',
            'Implement this workflow with your students',
            'Set up the classroom as follows',
            'Ensure students have notebooks ready',
            'Ensure teachers have all materials',
            'Prepare the students for the next activity',
        ];

        foreach ($testCases as $text) {
            $violations = $this->callPrivateMethod(MediaGeneratedContentGuard::class, 'assertTextSafe', ['test_field', $text]);
            $this->assertNotEmpty($violations, "Should reject procedural instruction: $text");
            $this->assertEquals('procedural_instruction', $violations[0]['pattern_name']);
        }
    }

    public function test_it_accepts_legitimate_student_facing_instructions()
    {
        $testCases = [
            'Students will solve the following problems',
            'Work through example 1 together',
            'Ensure accuracy of calculations',
            'Create a presentation about your findings',
        ];

        foreach ($testCases as $text) {
            $violations = $this->callPrivateMethod(MediaGeneratedContentGuard::class, 'assertTextSafe', ['test_field', $text]);
            $this->assertEmpty($violations, "Should accept legitimate instruction: $text");
        }
    }

    public function test_it_rejects_conversational_filler()
    {
        $testCases = [
            'Here is your material for today',
            'I have generated a complete lesson plan',
            'I have prepared the exercises',
            'As an AI, I created the following structure',
            'As a language model, I suggest...',
            'According to my analysis, the lesson should be...',
        ];

        foreach ($testCases as $text) {
            $violations = $this->callPrivateMethod(MediaGeneratedContentGuard::class, 'assertTextSafe', ['test_field', $text]);
            $this->assertNotEmpty($violations, "Should reject conversational filler: $text");
            $this->assertEquals('conversational_filler', $violations[0]['pattern_name']);
        }
    }

    public function test_it_rejects_structural_scaffolding()
    {
        $testCases = [
            'This section is designed to teach algebra',
            'This lesson aims to explain photosynthesis',
            'This activity will cover quadratic equations',
            'Focus on the following three outcomes',
            'Be sure to emphasize the main point',
            'The purpose of this section is to...',
        ];

        foreach ($testCases as $text) {
            $violations = $this->callPrivateMethod(MediaGeneratedContentGuard::class, 'assertTextSafe', ['test_field', $text]);
            $this->assertNotEmpty($violations, "Should reject structural scaffolding: $text");
            $this->assertEquals('structural_scaffolding', $violations[0]['pattern_name']);
        }
    }

    public function test_it_accepts_legitimate_pedagogical_content()
    {
        $testCases = [
            'Here are two methods for solving quadratic equations',
            'Learning outcomes: Students can identify...',
            'The main concept of this lesson is...',
            'Quadratic equations are first-degree polynomial equations.',
        ];

        foreach ($testCases as $text) {
            $violations = $this->callPrivateMethod(MediaGeneratedContentGuard::class, 'assertTextSafe', ['test_field', $text]);
            $this->assertEmpty($violations, "Should accept legitimate pedagogical content: $text");
        }
    }

    public function test_it_reports_multiple_violations()
    {
        $text = "Here is your material. Follow these steps to implement.";
        $violations = $this->callPrivateMethod(MediaGeneratedContentGuard::class, 'assertTextSafe', ['test_field', $text]);
        
        $this->assertCount(2, $violations);
        $patterns = array_column($violations, 'pattern_name');
        $this->assertContains('conversational_filler', $patterns);
        $this->assertContains('procedural_instruction', $patterns);
    }

    public function test_it_validates_teacher_delivery_summary_length()
    {
        $shortText = "Short summary.";
        $violations = MediaGeneratedContentGuard::assertTeacherDeliverySummary('summary', $shortText);
        $this->assertEmpty($violations);

        $longText = str_repeat("This is a very long summary sentence that should eventually exceed two hundred characters so that we can test if the guard correctly identifies that it is too long for the summary field. ", 2);
        $violations = MediaGeneratedContentGuard::assertTeacherDeliverySummary('summary', $longText);
        $this->assertNotEmpty($violations);
        $this->assertEquals('excessive_delivery_summary_length', $violations[0]['pattern_name']);
    }

    public function test_it_validates_learning_objective_perspective()
    {
        $studentFacing = "Students will understand photosynthesis.";
        $violations = MediaGeneratedContentGuard::assertLearningObjective('obj', $studentFacing);
        $this->assertEmpty($violations);

        $teacherFacing = "Teacher will explain photosynthesis.";
        $violations = MediaGeneratedContentGuard::assertLearningObjective('obj', $teacherFacing);
        $this->assertNotEmpty($violations);
        $this->assertEquals('procedural_instruction', $violations[0]['pattern_name']);
    }

    /**
     * Helper to call private/protected methods.
     */
    protected function callPrivateMethod($class, $method, array $args)
    {
        $reflection = new \ReflectionClass($class);
        $method = $reflection->getMethod($method);
        $method->setAccessible(true);
        return $method->invokeArgs(null, $args);
    }
}
