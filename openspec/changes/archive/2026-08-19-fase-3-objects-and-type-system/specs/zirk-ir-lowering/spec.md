## ADDED Requirements

### Requirement: Lowering de objetos

El lowering SHALL traducir la construcción de un objeto a la operación abstracta de alocación seguida de la inicialización de sus campos, y el acceso a un campo a una lectura por desplazamiento.

#### Scenario: Construcción
- **WHEN** se baja `User(1)`
- **THEN** se emite la alocación abstracta del tipo
- **AND** el cuerpo del constructor inicializa los campos

#### Scenario: Acceso a un campo
- **WHEN** se baja `u.id`
- **THEN** se emite una lectura en el desplazamiento del campo, sin búsqueda por nombre

### Requirement: Lowering de llamadas a métodos

El lowering SHALL emitir una llamada directa cuando el método no es redefinible, y una llamada indirecta a través de la tabla del tipo cuando lo es.

La mayoría de las llamadas son del primer caso, y pagar una indirección por todas ellas sería pagar por una generalidad que el programa no usa.

#### Scenario: Método no redefinido
- **WHEN** ninguna subclase redefine el método llamado
- **THEN** se emite una llamada directa

#### Scenario: Método redefinido
- **WHEN** alguna subclase lo redefine
- **THEN** se emite una llamada indirecta a través de la tabla del tipo

#### Scenario: Llamada a través de un contrato
- **WHEN** el receptor tiene el tipo de una interfaz
- **THEN** se despacha por la tabla de esa interfaz

### Requirement: Lowering de acceso seguro

El lowering SHALL traducir `expr?.miembro` a una comprobación explícita de nulidad con dos bloques: el presente accede al miembro y el ausente produce el valor nulo.

Reutiliza el mecanismo que `??` introdujo en la fase anterior; lo que llega es el operador, no la maquinaria.

#### Scenario: Acceso seguro
- **WHEN** se baja `usuario?.nombre`
- **THEN** se produce una comprobación de nulidad con un bloque por cada resultado

#### Scenario: Encadenado
- **WHEN** se baja `a?.b?.c`
- **THEN** cada eslabón comprueba antes de acceder

### Requirement: Especialización de genéricos

El lowering SHALL producir una función por combinación de argumentos de tipo usada, y reutilizarla cuando la combinación se repite.

#### Scenario: Una copia por combinación
- **WHEN** una función genérica se usa con dos combinaciones distintas
- **THEN** la IR contiene dos funciones

#### Scenario: Sin duplicados
- **WHEN** la misma combinación se usa varias veces
- **THEN** la IR contiene una sola función para ella
