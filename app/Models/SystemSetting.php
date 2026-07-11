<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;

class SystemSetting extends Model
{
    protected $fillable = ['key', 'value', 'type', 'group', 'description'];

    /**
     * Get a setting value by key.
     * 
     * @param string $key
     * @param mixed $default
     * @return mixed
     */
    public static function get($key, $default = null)
    {
        $setting = self::where('key', $key)->first();

        if (!$setting) {
            return $default;
        }

        $value = $setting->value;

        // Cast value based on type
        switch ($setting->type) {
            case 'boolean':
            case 'bool':
                return filter_var($value, FILTER_VALIDATE_BOOLEAN);
            case 'number':
            case 'integer':
            case 'int':
                return (int) $value;
            case 'json':
            case 'array':
                return json_decode($value, true);
            default:
                return $value;
        }
    }

    /**
     * Set a setting value by key.
     * 
     * @param string $key
     * @param mixed $value
     * @param string|null $type
     * @return bool
     */
    public static function set($key, $value, $type = null)
    {
        $setting = self::where('key', $key)->first();

        if (!$setting) {
            $setting = new self();
            $setting->key = $key;
            if ($type) {
                $setting->type = $type;
            }
        }

        if (is_array($value) || is_object($value)) {
            $setting->value = json_encode($value);
            if (!$setting->type) $setting->type = 'json';
        } else {
            $setting->value = (string) $value;
        }

        return $setting->save();
    }
}
