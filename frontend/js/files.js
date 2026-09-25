// Lê o primeiro arquivo de um <input type="file"> como base64 (sem o prefixo "data:...").
export function readFileBase64(input) {
  const file = input.files && input.files[0];
  if (!file) return Promise.reject('Nenhum arquivo selecionado');
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result).split(',', 2)[1] ?? '');
    reader.onerror = () => reject('Não foi possível ler o arquivo');
    reader.readAsDataURL(file);
  });
}

export function fileName(input) {
  return (input.files && input.files[0] && input.files[0].name) || '';
}
