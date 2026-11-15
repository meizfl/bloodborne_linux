"""Stable resource identifiers for reviewable, reproducible UI exports."""
import re

COMMAND = re.compile(r'\d+(?:<lambda>|[A-Za-z_]\w*)')
OBJECT_ADDRESS = re.compile(r' at 0x[0-9a-fA-F]+(?=>)')


def normalize(manifest):
    mapping = {name: f'bb::callback{index:04}' for index, name in enumerate(manifest['callbacks'])
               if COMMAND.fullmatch(name)}

    def text(value):
        value = COMMAND.sub(lambda match: mapping.get(match.group(), match.group()), value)
        return OBJECT_ADDRESS.sub(' at 0xOBJECT', value)

    def walk(value):
        if isinstance(value, str):
            return text(value)
        if isinstance(value, list):
            return [walk(item) for item in value]
        if isinstance(value, dict):
            return {text(key): walk(item) for key, item in value.items()}
        return value

    return walk(manifest)
